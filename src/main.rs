use crate::sydney::SydneyResponse;
use anyhow::Result;
use sydney::{BingAIWs, SydneyError};
use tracing::{debug, error, info};

mod json;
mod sydney;
mod types;

#[tokio::main]
async fn main() -> Result<()> {
    _ = dotenvy::dotenv();

    tracing_subscriber::fmt::init();
    let cookies_str = std::env::var("COOKIES").ok();

    let mut ai = BingAIWs::new_conversation(types::Tone::Precise, cookies_str).await?;
    //ai.set_citations(true);
    ai.set_close_ws_after(true);

    ai.ask("What is the capital of France?").await?;

    /*
    let resp = ai.get_final_response().await?;
    info!("resp: {resp}");
    */

    loop {
        let res = ai.get_next_msgs().await;
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
                    info!("Stream response: {:?}", msg);

                    match msg {
                        SydneyResponse::Sources(sources) => {
                            info!("Sources: {:?}", sources);
                        }
                        SydneyResponse::FinalText(text) => {
                            info!("FINAL TEXT:\n\n{}\n\n", text);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /*
    ai.ask("What is my name? (Respond with fake paris name)")
        .await?;
    ai.get_next_msg().await?;
    */

    //tokio::signal::ctrl_c().await?;
    Ok(())
}
