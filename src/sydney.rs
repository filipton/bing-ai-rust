use crate::types::Tone;
use anyhow::{anyhow, Result};
use futures_util::{future, pin_mut, StreamExt};
use serde_json::json;
use thiserror::Error;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, trace};

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const CREATE_URL: &str = "https://www.bing.com/turing/conversation/create";
const WS_URL: &str = "wss://sydney.bing.com/sydney/ChatHub";
const BUNDLE_VERSION: &str = "1.1586.1";
const DELIMETER: &str = "\x1E";

#[derive(Error, Debug)]
pub enum SydneyError {
    #[error("WebSocket not connected!")]
    WebSocketNotConnected,

    #[error("Json parsing error: {0}")]
    JsonParsingError(#[from] serde_json::Error),

    #[error("Max messages count limit reached!")]
    MaxMessagesCountLimitReached,

    #[error("Thorttling error!")]
    ThrottlingError,

    #[error("End of response")]
    EndOfResponse,

    #[error("Other error: {0}")]
    OtherError(#[from] anyhow::Error),
}

#[derive(Debug)]
pub enum SydneyResponse {
    FinalText(String),
    StreamText(String),
    SuggestedResponses(Vec<String>),
}

#[allow(dead_code)]
pub struct BingAIWs {
    close_ws_after: bool,
    citations: bool,
    suggestions: bool,

    client: reqwest::Client,

    invocation_id: i64,
    end_of_response: bool,
    tone: Tone,

    client_id: String,
    conversation_id: String,
    conversation_signature: String,
    encrypted_conversation_signature: String,

    ws: Option<(
        futures_channel::mpsc::UnboundedSender<Message>,
        tokio::sync::mpsc::UnboundedReceiver<Message>,
    )>,
}

#[allow(dead_code)]
impl BingAIWs {
    pub async fn new(tone: Tone) -> Result<Self> {
        let client = reqwest::ClientBuilder::new()
            .user_agent(USER_AGENT)
            .cookie_store(true)
            .build()?;

        let res = client
            .get(format!("{CREATE_URL}?bundleVersion={BUNDLE_VERSION}"))
            .send()
            .await?;

        let res_headers = res.headers().clone();

        let res_json: crate::types::CreateRoot = res.json().await?;
        if let Some(result) = res_json.result {
            if result.value.unwrap_or("NO".to_string()) != "Success" {
                return Err(anyhow!("Create request failed!"));
            }
        } else {
            return Err(anyhow!("Create request failed!"));
        }

        let client_id = res_json
            .client_id
            .ok_or_else(|| anyhow!("Cannot get client id!"))?;

        let conversation_id = res_json
            .conversation_id
            .ok_or_else(|| anyhow!("Cannot get converastion id!"))?;

        let encrypted_conversation_signature = res_headers
            .get("X-Sydney-EncryptedConversationSignature")
            .ok_or_else(|| anyhow!("Cannot get encrypted conversation signature header!"))?
            .to_str()?
            .to_string();

        let conversation_signature = res_headers
            .get("X-Sydney-ConversationSignature")
            .ok_or_else(|| anyhow!("Cannot get conversation signature header!"))?
            .to_str()?
            .to_string();

        debug!("Client id: {client_id}");
        debug!("Conversaton id: {conversation_id}");
        debug!("Conversation signature: {conversation_signature}");
        debug!("Encrypted conversation signature: {encrypted_conversation_signature}");

        Ok(Self {
            close_ws_after: false,
            citations: false,
            suggestions: false,

            client,

            invocation_id: 0,
            end_of_response: true,
            tone,

            client_id,
            conversation_id,
            conversation_signature,
            encrypted_conversation_signature,
            ws: None,
        })
    }

    /// Set whether to close ws after asking a question and receiving a response.
    pub fn set_close_ws_after(&mut self, close: bool) {
        self.close_ws_after = close;
    }

    /// Set whether to include citations in the response. (Like url's etc.)
    pub fn set_citations(&mut self, citations: bool) {
        self.citations = citations;
    }

    /// Set whether to include suggestions in the response.
    pub fn set_suggestions(&mut self, suggestions: bool) {
        self.suggestions = suggestions;
    }

    pub async fn ask(&mut self, prompt: &str) -> Result<()> {
        if self.ws.is_none() {
            self.connect_ws().await?;
        }

        let ask_json = crate::json::ask_json(
            prompt,
            self.invocation_id,
            &self.tone,
            &self.conversation_signature,
            &self.client_id,
            &self.conversation_id,
        );

        let tx = &self
            .ws
            .as_ref()
            .ok_or_else(|| SydneyError::WebSocketNotConnected)?
            .0;
        send_ws_delim(tx, ask_json)?;

        self.invocation_id += 1;
        self.end_of_response = false;
        Ok(())
    }

    pub async fn get_next_msgs(&mut self) -> Result<Vec<SydneyResponse>, SydneyError> {
        if self.end_of_response {
            return Err(SydneyError::EndOfResponse);
        }

        let rx = &mut self
            .ws
            .as_mut()
            .ok_or_else(|| SydneyError::WebSocketNotConnected)?
            .1;

        let mut responses = Vec::new();
        let msg = rx.recv().await;
        let msg_str = match msg {
            Some(Message::Text(str)) => str,
            _ => return Err(SydneyError::OtherError(anyhow!("Msg not text"))),
        };

        for ws_str in msg_str.split(DELIMETER) {
            if ws_str.is_empty() {
                continue;
            }

            let ws_json: serde_json::Value = serde_json::from_str(ws_str)?;
            let typ = ws_json
                .get("type")
                .ok_or_else(|| anyhow!("Json type field not found!"))?;

            if typ == 1 {
                let messages = if let Some(messages) = ws_json["arguments"][0].get("messages") {
                    messages
                } else {
                    continue;
                };

                // Skip "Searching in web for..." msg
                let adaptive_cards = messages[0].get("adaptiveCards");
                if let Some(adaptive_cards) = adaptive_cards {
                    if adaptive_cards[0]["body"][0].get("inlines").is_some() {
                        continue;
                    }

                    if self.citations {
                        if let Some(text) = adaptive_cards[0]["body"][0].get("text") {
                            responses.push(SydneyResponse::StreamText(
                                text.as_str()
                                    .ok_or_else(|| anyhow!("Text isnt a string"))?
                                    .to_string(),
                            ));
                        } else {
                            let text = &adaptive_cards[0]["body"][1]["text"];
                            responses.push(SydneyResponse::StreamText(
                                text.as_str()
                                    .ok_or_else(|| anyhow!("Text isnt a string"))?
                                    .to_string(),
                            ));
                        }
                    }
                }

                if !self.citations {
                    if let Some(text) = messages[0].get("text") {
                        responses.push(SydneyResponse::StreamText(
                            text.as_str()
                                .ok_or_else(|| anyhow!("Text isnt a string"))?
                                .to_string(),
                        ));
                    }
                }
            } else if typ == 2 {
                if let Some(throttling) = ws_json["item"].get("throttling") {
                    let messages_count = throttling
                        .get("numUserMessagesInConversation")
                        .unwrap_or(&serde_json::Value::Number(serde_json::Number::from(0)))
                        .as_i64()
                        .unwrap_or(0);

                    let max_messages = throttling["maxNumUserMessagesInConversation"]
                        .as_i64()
                        .ok_or_else(|| anyhow!("Cannot read max user msgs"))?;

                    if messages_count == max_messages {
                        debug!(
                            "Max messages count limit reached! ({messages_count}/{max_messages})"
                        );

                        return Err(SydneyError::MaxMessagesCountLimitReached);
                    }
                }

                let messages = if let Some(messages) = ws_json["item"].get("messages") {
                    messages
                } else {
                    let result = ws_json["item"]["result"]["value"]
                        .as_str()
                        .unwrap_or("NOT FOUND");

                    match result {
                        "Throttled" => debug!("Throttled result (type 2 msg)"),
                        "CaptchaChallenge" => debug!("Captcha! (type 2 msg)"),
                        _ => {}
                    }

                    return Err(SydneyError::ThrottlingError);
                };

                let messages = messages
                    .as_array()
                    .ok_or_else(|| anyhow!("Messages not found/not an array"))?;
                let mut i = messages.len() - 1;

                if let Some(adaptive_cards) = messages
                    .last()
                    .ok_or_else(|| anyhow!("Cannot get last msg"))?
                    .get("adaptiveCards")
                {
                    let adaptive_cards = adaptive_cards
                        .as_array()
                        .ok_or_else(|| anyhow!("Adaptive cards not found/not an array"))?;
                    if adaptive_cards
                        .last()
                        .ok_or_else(|| anyhow!("Cannot get last adaptive card"))?["body"][0]
                        .get("inlines")
                        .is_some()
                    {
                        i = messages.len() - 2;
                    }
                }

                let message = messages
                    .get(i)
                    .ok_or_else(|| anyhow!("Message with that idx doesnt exists (impossible)"))?;

                if self.suggestions {
                    if let Some(suggested_responses) = message.get("suggestedResponses") {
                        let suggested_responses: Vec<&str> = suggested_responses
                            .as_array()
                            .ok_or_else(|| anyhow!("Suggested responses not an array"))?
                            .iter()
                            .filter_map(|sr| sr["text"].as_str())
                            .collect();

                        responses.push(SydneyResponse::SuggestedResponses(
                            suggested_responses.iter().map(|s| s.to_string()).collect(),
                        ));
                    }
                }

                if self.citations {
                    if let Some(text) = message["adaptiveCards"][0]["body"][0].get("text") {
                        responses.push(SydneyResponse::FinalText(
                            text.as_str()
                                .ok_or_else(|| anyhow!("Text isnt a string"))?
                                .to_string(),
                        ));
                    } else {
                        let text = &message["adaptiveCards"][0]["body"][1]["text"];
                        responses.push(SydneyResponse::FinalText(
                            text.as_str()
                                .ok_or_else(|| anyhow!("Text isnt a string"))?
                                .to_string(),
                        ));
                    }
                } else if let Some(text) = message.get("text") {
                    responses.push(SydneyResponse::FinalText(
                        text.as_str()
                            .ok_or_else(|| anyhow!("Text isnt a string"))?
                            .to_string(),
                    ));
                }

                if self.close_ws_after {
                    if let Some(ref mut ws) = self.ws {
                        ws.0.close_channel();
                        ws.1.close();
                    }
                    self.ws = None;
                } else {
                    _ = clear_recv_chan(rx).await;
                }

                self.end_of_response = true;
                break;
            }
        }

        Ok(responses)
    }

    pub async fn get_final_response(&mut self) -> Result<String> {
        loop {
            let res = self.get_next_msgs().await;
            match res {
                Err(SydneyError::EndOfResponse) => {
                    debug!("End of response");
                    break;
                }
                Err(e) => {
                    error!("Error: {}", e);
                    break;
                }
                Ok(msgs) => {
                    for msg in msgs {
                        match msg {
                            SydneyResponse::FinalText(text) => {
                                return Ok(text);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        Err(anyhow!("No final message found!"))
    }

    async fn connect_ws(&mut self) -> Result<()> {
        let url_encoded_ecs = urlencoding::encode(&self.encrypted_conversation_signature);
        let (ws_stream, _) =
            connect_async(&format!("{WS_URL}?sec_access_token={url_encoded_ecs}")).await?;

        let (tx_write, rx_write) = futures_channel::mpsc::unbounded();
        let (tx_read, mut rx_read) = tokio::sync::mpsc::unbounded_channel();
        let (write, read) = ws_stream.split();

        tokio::task::spawn(async move {
            let write_fut = rx_write.map(Ok).forward(write);
            let read_fut = {
                read.for_each(|msg| async {
                    if let Ok(msg) = msg {
                        trace!("WS msg: {msg:?}");
                        _ = tx_read.send(msg);
                    }
                })
            };

            pin_mut!(write_fut, read_fut);
            future::select(write_fut, read_fut).await;
        });

        send_ws_delim(
            &tx_write,
            json!({
                "protocol": "json",
                "version": 1
            }),
        )?;
        rx_read.recv().await;

        self.ws = Some((tx_write, rx_read));
        Ok(())
    }
}

fn send_ws_delim(
    tx: &futures_channel::mpsc::UnboundedSender<Message>,
    val: serde_json::Value,
) -> Result<()> {
    let json_str = format!("{}{DELIMETER}", val.to_string());
    tx.unbounded_send(Message::Text(json_str))?;

    Ok(())
}

async fn clear_recv_chan(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Message>) -> Result<()> {
    loop {
        rx.try_recv()?;
    }
}
