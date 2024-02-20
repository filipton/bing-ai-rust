use anyhow::Result;
use std::time::Instant;
use sydney::{BingAIWs, SydneyError};
use tracing::{debug, error, info};

use crate::sydney::SydneyResponse;

mod json;
mod sydney;
mod types;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let mut ai = BingAIWs::new(types::Tone::Precise).await?;
    //ai.set_citations(true);
    ai.set_close_ws_after(true);

    ai.ask("How can i make simple multi-threaded http server in plain C? DO NOT ANWSER WITH QUESTION. WRITE THE ANWSER ONLY. Use README syntax for codeblocks - code blocks should be inside \"```\" with approtiate language.").await?;

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
                    info!("Response: {:?}", msg);

                    if let SydneyResponse::FinalText(text) = msg {
                        println!("{}", text);
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
