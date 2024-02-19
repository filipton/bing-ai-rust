use crate::types::Tone;
use anyhow::{anyhow, Result};
use futures_util::{future, pin_mut, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::debug;

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const CREATE_URL: &str = "https://www.bing.com/turing/conversation/create";
const WS_URL: &str = "wss://sydney.bing.com/sydney/ChatHub";
const BUNDLE_VERSION: &str = "1.1586.1";
const DELIMETER: &str = "\x1E";

pub struct BingAIWs {
    invocation_id: i64,
    tone: Tone,

    client_id: String,
    conversation_id: String,
    conversation_signature: String,

    ws: (
        futures_channel::mpsc::UnboundedSender<Message>,
        tokio::sync::mpsc::UnboundedReceiver<Message>,
    ),
}

impl BingAIWs {
    pub async fn new(tone: Tone) -> Result<Self> {
        let client = reqwest::ClientBuilder::new()
            .user_agent(USER_AGENT)
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
            .to_str()?;

        let conversation_signature = res_headers
            .get("X-Sydney-ConversationSignature")
            .ok_or_else(|| anyhow!("Cannot get conversation signature header!"))?
            .to_str()?
            .to_string();

        debug!("Client id: {client_id}");
        debug!("Conversaton id: {conversation_id}");
        debug!("Conversation signature: {conversation_signature}");
        debug!("Encrypted conversation signature: {encrypted_conversation_signature}");

        let url_encoded_ecs = urlencoding::encode(encrypted_conversation_signature);
        let (tx, mut rx) =
            connect_ws(&format!("{WS_URL}?sec_access_token={url_encoded_ecs}")).await?;

        send_ws_delim(
            &tx,
            json!({
                "protocol": "json",
                "version": 1
            }),
        )?;
        rx.recv().await;

        Ok(Self {
            invocation_id: 0,
            tone,

            client_id,
            conversation_id,
            conversation_signature,
            ws: (tx, rx),
        })
    }

    pub async fn ask(&mut self, prompt: &str) -> Result<()> {
        let ask_json = crate::json::ask_json(
            prompt,
            self.invocation_id,
            &self.tone,
            &self.conversation_signature,
            &self.client_id,
            &self.conversation_id,
        );
        send_ws_delim(&self.ws.0, ask_json)?;

        self.invocation_id += 1;
        Ok(())
    }
}

async fn connect_ws(
    url: &str,
) -> Result<(
    futures_channel::mpsc::UnboundedSender<Message>,
    tokio::sync::mpsc::UnboundedReceiver<Message>,
)> {
    let (ws_stream, _) = connect_async(url).await?;

    let (tx_write, rx_write) = futures_channel::mpsc::unbounded();
    let (tx_read, rx_read) = tokio::sync::mpsc::unbounded_channel();
    let (write, read) = ws_stream.split();

    tokio::task::spawn(async move {
        let write_fut = rx_write.map(Ok).forward(write);
        let read_fut = {
            read.for_each(|msg| async {
                if let Ok(msg) = msg {
                    debug!("WS msg: {msg:?}");
                    _ = tx_read.send(msg);
                }
            })
        };

        pin_mut!(write_fut, read_fut);
        future::select(write_fut, read_fut).await;
    });
    Ok((tx_write, rx_read))
}

fn send_ws_delim(
    tx: &futures_channel::mpsc::UnboundedSender<Message>,
    val: serde_json::Value,
) -> Result<()> {
    let json_str = format!("{}{DELIMETER}", val.to_string());
    tx.unbounded_send(Message::Text(json_str))?;

    Ok(())
}
