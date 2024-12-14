use log::info;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub port: u16,
    pub host: String,
}

pub async fn connect_to_nats(
    config: Config,
) -> Result<async_nats::Client, Box<dyn std::error::Error>> {
    let connection_string = format!("nats://{}:{}", config.host, config.port);

    info!("Connecting to {}", connection_string);

    let client = async_nats::connect(connection_string).await?;

    Ok(client)
}
