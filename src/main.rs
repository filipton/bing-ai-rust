use anyhow::Result;
use sydney::BingAIWs;

mod json;
mod sydney;
mod types;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let mut ai = BingAIWs::new(types::Tone::Precise).await?;
    ai.ask("My name is \"Filip\". Please respond with it when i'll ask you to do it!")
        .await?;
    ai.parse_msgs().await?;
    ai.ask("What is my name?").await?;
    ai.parse_msgs().await?;

    tokio::signal::ctrl_c().await?;
    Ok(())
}
