use std::fs::File;
use std::io::Read;

use serde::Deserialize;

use crate::grpc;
use crate::metrics;
use crate::nats;
use crate::postgres;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub automigrate: bool,
    pub log_level: String,
    pub metrics: Metrics,
    pub repository: Repository,
    pub reset_state: bool,
    pub transmitter: Transmitter,
    pub transport: Transport,
}

#[derive(Debug, Deserialize)]
pub enum Transport {
    Grpc(grpc::Config),
}

#[derive(Debug, Deserialize)]
pub enum Transmitter {
    Nats(nats::Config),
}

#[derive(Debug, Deserialize)]
pub enum Repository {
    Postgres(postgres::Config),
    InMemory,
}

#[derive(Debug, Deserialize)]
pub enum Metrics {
    Prometheus(metrics::Config),
}

pub fn load_config_from_file(file_path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: Config = ron::de::from_str(&contents)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config() {
        // The sample config is checked in to version control, so must always be up-to-date.
        let config_file = "./sample.ron";
        let config = load_config_from_file(config_file).expect("could not load configuration");

        // Merely asserting the log level is enough to assert the structure of the file contents.
        assert_eq!(config.log_level, "debug".to_string());
    }
}
