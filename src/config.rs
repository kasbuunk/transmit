use core::time;

use serde::Deserialize;

use crate::grpc;
use crate::metrics;
use crate::nats;
use crate::postgres;

#[derive(Debug, Clone)]
pub struct Config {
    pub automigrate: bool,
    pub log_level: log::Level,
    pub clock_cycle_interval: time::Duration,
    pub metrics: Metrics,
    pub repository: Repository,
    pub reset_state: bool,
    pub transmitter: Transmitter,
    pub transport: Transport,
}

#[derive(Debug, Clone, Deserialize)]
pub enum Transport {
    Grpc(grpc::Config),
}

#[derive(Debug, Clone, Deserialize)]
pub enum Transmitter {
    Nats(nats::Config),
}

#[derive(Debug, Clone, Deserialize)]
pub enum Repository {
    Postgres(postgres::Config),
    InMemory,
}

#[derive(Debug, Clone, Deserialize)]
pub enum Metrics {
    Prometheus(metrics::Config),
}
