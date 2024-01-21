use log::info;
use serde::Deserialize;
use sqlx::postgres::{PgPool, PgPoolOptions};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub ssl: bool,
}

pub async fn connect_to_database(config: Config) -> Result<PgPool, Box<dyn std::error::Error>> {
    let connection_string = format!(
        "postgres://{}:{}@{}/{}",
        config.user, config.password, config.host, config.name
    );

    info!(
        "Connecting to postgres://{}:[redacted]@{}/{}",
        config.user, config.host, config.name
    );

    let connection_pool = PgPoolOptions::new().connect(&connection_string).await?;
    return Ok(connection_pool);
}
