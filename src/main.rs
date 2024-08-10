mod config;
mod message_bus;
mod utils;

use crate::config::Config;
use crate::message_bus::MessageBus;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new();
    let message_bus = MessageBus::new(config);
    message_bus.run().await?;
    Ok(())
}
