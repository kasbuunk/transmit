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

    Ok(connection_pool)
}

#[cfg(test)]
pub fn random_db_name(name: &str) -> String {
    format!("{}_test_{}", name, uuid::Uuid::new_v4().as_simple())
}

#[cfg(test)]
pub async fn connect_to_test_database(
    config: Config,
) -> Result<PgPool, Box<dyn std::error::Error>> {
    use sqlx::postgres::PgConnectOptions;

    let mut options = PgConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .username(&config.user)
        .password(&config.password);

    // If SSL is enabled, set SSL mode
    if config.ssl {
        options = options.ssl_mode(sqlx::postgres::PgSslMode::Require);
    }

    // Connect to the PostgreSQL server
    let pool = PgPoolOptions::new()
        .max_connections(50) // Set the maximum number of connections in the pool
        .connect_with(options)
        .await?;

    // Create the database if it doesn't exist
    let db_name = random_db_name(&config.name);

    let _ignore_db_already_exists_err = sqlx::query(&format!("CREATE DATABASE {};", db_name))
        .execute(&pool)
        .await;

    let test_config = Config {
        name: db_name,
        host: config.host,
        port: config.port,
        user: config.user,
        password: config.password,
        ssl: config.ssl,
    };

    connect_to_database(test_config).await
}
