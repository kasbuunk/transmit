use std::fs::File;
use std::io::Read;

use serde::Deserialize;

use crate::grpc;
use crate::metrics;
use crate::postgres;
use crate::transmitter_nats;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub log_level: String,
    pub transmitter: Transmitter,
    pub repository: Repository,
    pub transport: Transport,
    pub metrics: Metrics,
}

#[derive(Debug, Deserialize)]
pub enum Transport {
    Grpc(grpc::Config),
}

#[derive(Debug, Deserialize)]
pub enum Transmitter {
    Nats(transmitter_nats::Config),
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
        let config = load_config_from_file("./sample.ron").expect("could not load configuration");

        // Merely asserting the log level is enough to assert the structure of the configuration
        // file.
        assert_eq!(config.log_level, "debug".to_string());
    }
}
