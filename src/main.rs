use anyhow::Result;
use sydney::{BingAIWs, SydneyError};
use tracing::{debug, error};

mod json;
mod sydney;
mod types;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let mut ai = BingAIWs::new(types::Tone::Precise).await?;
    //ai.set_citations(true);
    ai.set_close_ws_after(true);

    ai.ask("What the weather today in Paris?").await?;
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
            Ok(msg) => {
                println!("Response: {:?}", msg);
            }
        }
    }

    /*
    ai.ask("What is my name? (Respond with fake paris name)")
        .await?;
    ai.get_next_msg().await?;
    */

    tokio::signal::ctrl_c().await?;
    Ok(())
}
