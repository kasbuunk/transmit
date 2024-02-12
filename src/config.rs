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
