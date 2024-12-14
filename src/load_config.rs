use humantime;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;

use serde::Deserialize;

use crate::config;
use crate::postgres;

const ENV_POSTGRES_PASSWORD: &'static str = "POSTGRES_PASSWORD";

#[derive(Debug, Deserialize)]
struct FileConfig {
    automigrate: bool,
    log_level: String,
    clock_cycle_interval: String,
    metrics: config::Metrics,
    repository: Repository,
    reset_state: bool,
    transmitter: config::Transmitter,
    transport: config::Transport,
}

#[derive(Debug, Deserialize)]
enum Repository {
    Postgres(Postgres),
    InMemory,
}

#[derive(Debug, Deserialize)]
struct Postgres {
    name: String,
    host: String,
    port: u16,
    user: String,
    ssl: bool,
}

fn load_config_from_file(file_path: &str) -> Result<FileConfig, Box<dyn Error>> {
    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: FileConfig = ron::de::from_str(&contents)?;
    Ok(config)
}

struct EnvConfig {
    postgres_password: String,
}

fn load_secrets_from_env() -> Result<EnvConfig, Box<dyn Error>> {
    let postgres_password = env::var(ENV_POSTGRES_PASSWORD)?;

    Ok(EnvConfig { postgres_password })
}

pub fn load_config(file_path: &str) -> Result<config::Config, Box<dyn Error>> {
    let config = load_config_from_file(file_path)?;

    let secrets = load_secrets_from_env()?;

    let config = derive_config(config, secrets)?;

    config::validate(&config)?;

    Ok(config)
}

fn derive_config(config: FileConfig, secrets: EnvConfig) -> Result<config::Config, Box<dyn Error>> {
    let log_level = match log::Level::from_str(&config.log_level) {
        Ok(log_level) => log_level,
        Err(err) => {
            return Err(format!("invalid log level '{}': {err}", &config.log_level).into());
        }
    };

    let clock_cycle_interval = humantime::parse_duration(&config.clock_cycle_interval)?;

    Ok(config::Config {
        automigrate: config.automigrate,
        log_level,
        clock_cycle_interval,
        metrics: config.metrics,
        repository: match config.repository {
            Repository::Postgres(postgres_config) => {
                config::Repository::Postgres(postgres::Config {
                    name: postgres_config.name,
                    host: postgres_config.host,
                    port: postgres_config.port,
                    user: postgres_config.user,
                    password: secrets.postgres_password,
                    ssl: postgres_config.ssl,
                })
            }
            Repository::InMemory => config::Repository::InMemory,
        },
        transmitter: config.transmitter,
        transport: config.transport,
        reset_state: config.reset_state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time;

    #[test]
    fn test_load_config() {
        // The sample config is checked in to version control, so must always be up-to-date.
        let config_file = "./sample.ron";
        let postgres_password = "Welkom2014!";

        // No env variable is set yet, so expect an error.
        assert!(load_config(config_file).is_err());

        env::set_var(ENV_POSTGRES_PASSWORD, postgres_password);
        let configuration = load_config(config_file).expect("could not load configuration");

        // Merely asserting the log level is enough to assert the structure of the file contents.
        assert_eq!(configuration.log_level, log::Level::Debug);

        assert_eq!(
            configuration.clock_cycle_interval,
            time::Duration::from_millis(100)
        );

        match configuration.repository {
            config::Repository::Postgres(postgres_config) => {
                assert_eq!(postgres_config.password, postgres_password);
            }
            _ => panic!("file configures for a postgres repository"),
        };
    }
}
