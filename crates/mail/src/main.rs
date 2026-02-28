use anyhow::Result;
use mail_engine::Engine;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let engine = Engine::new("mail");
    engine.start().await?;

    Ok(())
}
