use std::env;
use std::process;

use log::info;

use message_scheduler::config;

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

    let config = config::load_config_from_file(config_file_path)?;

    let rust_log = "RUST_LOG";

    env::set_var(rust_log, config.log_level);
    env_logger::init();

    info!("Starting application.");

    // TODO
    // Load configuration.
    // Construct transmitter.
    // Construct repository.
    // Construct metrics client and server.
    // Start metrics server.
    // Construct scheduler.
    // Construct gRPC server.
    // Start gRPC server.

    Ok(())
}
