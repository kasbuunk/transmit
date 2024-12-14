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

pub fn validate(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    if config.clock_cycle_interval.is_zero() {
        return Err("clock cycle interval cannot be zero".into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            automigrate: true,
            log_level: log::Level::Info,
            clock_cycle_interval: time::Duration::from_millis(100),
            metrics: Metrics::Prometheus(metrics::Config {
                port: 3000,
                endpoint: String::from("/metrics"),
            }),
            repository: Repository::InMemory,
            reset_state: true,
            transmitter: Transmitter::Nats(nats::Config {
                port: 3001,
                host: String::from("127.0.0.1"),
            }),
            transport: Transport::Grpc(grpc::Config { port: 3002 }),
        }
    }

    #[test]
    fn test_validate_config() {
        struct TestCase {
            name: String,
            config: Config,
            expected_valid: bool,
        }

        let test_cases = vec![
            TestCase {
                name: String::from("valid"),
                config: config(),
                expected_valid: true,
            },
            TestCase {
                name: String::from("zero duration"),
                config: Config {
                    clock_cycle_interval: time::Duration::from_secs(0),
                    ..config()
                },
                expected_valid: false,
            },
        ];

        for test_case in test_cases {
            let valid = validate(&test_case.config).is_ok();
            assert_eq!(
                valid, test_case.expected_valid,
                "test case failed: {}",
                test_case.name
            );
        }
    }
}
