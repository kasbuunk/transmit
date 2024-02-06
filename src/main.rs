use std::env;
use std::process;
use std::sync::Arc;

use chrono::prelude::*;
use log::info;

use message_scheduler::config;
use message_scheduler::contract;
use message_scheduler::grpc;
use message_scheduler::metrics;
use message_scheduler::nats;
use message_scheduler::postgres;
use message_scheduler::repository_in_memory;
use message_scheduler::repository_postgres;
use message_scheduler::scheduler;
use message_scheduler::transmitter_nats;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let config_file_path = match args.len() {
        1 => "config.ron", // Default configuration file.
        2 => &args[1],
        _ => {
            println!("Please specify the path to the configuration file as the only argument.");

            process::exit(1);
        }
    };

    // Load configuration.
    let config = config::load_config_from_file(config_file_path)?;

    // Initialise logger.
    let rust_log = "RUST_LOG";
    env::set_var(rust_log, config.log_level);
    env_logger::init();
    info!("Starting application.");

    // Construct transmitter.
    let transmitter: Arc<dyn contract::Transmitter> = match config.transmitter {
        config::Transmitter::Nats(nats_config) => {
            let nats_client = nats::connect_to_nats(nats_config).await?;
            let transmitter = transmitter_nats::NatsPublisher::new(nats_client);
            info!("Initialised nats transmitter.");

            Arc::new(transmitter)
        }
    };

    // Construct repository.
    let repository: Arc<dyn contract::Repository> = match config.repository {
        config::Repository::Postgres(postgres_config) => {
            let postgres_connection = postgres::connect_to_database(postgres_config).await?;
            let repository = repository_postgres::RepositoryPostgres::new(postgres_connection);

            info!("Initialised postgres repository.");

            if config.automigrate {
                info!("Running migrations.");

                repository.migrate().await?;
            }

            if config.reset_state {
                repository.clear_all().await?;
            }

            Arc::new(repository)
        }
        config::Repository::InMemory => {
            let repository = repository_in_memory::RepositoryInMemory::new();
            info!("Initialised in-memory repository.");

            Arc::new(repository)
        }
    };

    // Construct metrics client and server.
    let metrics_client: Arc<dyn contract::Metrics> = match config.metrics {
        config::Metrics::Prometheus(metrics_config) => {
            let (metrics_client, metrics_server) = metrics::new(metrics_config);

            // Start metrics server.
            let _handle = tokio::spawn(async move {
                metrics_server
                    .start_server()
                    .await
                    .expect("prometheus server must start");
            });

            Arc::new(metrics_client)
        }
    };

    // Utc is the now provider, which accesses the current time through the OS.
    let now_provider = Arc::new(Utc::now);

    // Construct scheduler.
    let scheduler = Arc::new(scheduler::MessageScheduler::new(
        repository,
        transmitter,
        now_provider,
        metrics_client,
    ));

    let scheduler_running = scheduler.clone();
    let _handle = tokio::spawn(async move {
        scheduler_running.run().await;
    });

    // Construct transport.
    match config.transport {
        config::Transport::Grpc(grpc_config) => {
            let grpc_server = grpc::GrpcServer::new(grpc_config, scheduler);

            // Start gRPC server.
            grpc_server.serve().await?
        }
    }

    Ok(())
}
