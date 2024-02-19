use anyhow::Result;
use ws::BingAIWs;

mod json;
mod types;
mod ws;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let mut ai = BingAIWs::new(types::Tone::Precise).await?;
    ai.ask("Test").await?;

    tokio::signal::ctrl_c().await?;
    Ok(())
}
