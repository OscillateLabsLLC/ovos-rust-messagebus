mod config;
mod message_bus;
mod utils;

use config::Config;
use message_bus::MessageBus;

#[tokio::main]
async fn main() {
    let config = Config::new();
    let message_bus = MessageBus::new(config);
    message_bus.run().await;
}
