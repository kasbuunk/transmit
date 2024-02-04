use std::fs::File;
use std::io::Read;

use serde::Deserialize;

use crate::grpc;
use crate::postgres;
use crate::transmitter_nats;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub log_level: String,
    pub transmitter: Transmitter,
    pub repository: Repository,
    pub transport: Transport,
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

pub fn load_config_from_file(file_path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: Config = ron::de::from_str(&contents)?;
    Ok(config)
}
