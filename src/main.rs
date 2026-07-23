use ovos_messagebus::{Config, MessageBus};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::new();
    let message_bus = MessageBus::new(config);
    message_bus.run().await?;
    Ok(())
}
