use ovos_messagebus::{Config, MessageBus};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[cfg(unix)]
async fn shutdown_signal() -> std::io::Result<()> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result?,
        _ = terminate.recv() => {},
    }
    Ok(())
}

#[cfg(not(unix))]
async fn shutdown_signal() -> std::io::Result<()> {
    tokio::signal::ctrl_c().await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::new();
    let message_bus = MessageBus::new(config);
    tokio::select! {
        result = message_bus.run() => result?,
        result = shutdown_signal() => {
            result?;
            info!("Shutdown signal received");
        },
    }
    Ok(())
}
