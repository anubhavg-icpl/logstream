//! LogStream Server Binary
//!
//! High-performance centralized log aggregation server.

use clap::Parser;
use logstream::config::ServerConfig;
use logstream::server::LogServer;
use std::path::PathBuf;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "logstream-server")]
#[command(about = "High-performance centralized logging server")]
#[command(version)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config/server.toml")]
    config: PathBuf,

    /// Socket path to bind to
    #[arg(short, long)]
    socket: Option<String>,

    /// Log output directory
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Enable journald backend
    #[cfg(feature = "journald")]
    #[arg(long)]
    journald: bool,

    /// Enable metrics endpoint
    #[cfg(feature = "metrics")]
    #[arg(long)]
    metrics: bool,

    /// Metrics port
    #[cfg(feature = "metrics")]
    #[arg(long, default_value = "9090")]
    metrics_port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(if args.verbose {
            "logstream=debug,info"
        } else {
            "logstream=info,warn,error"
        })
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    info!("Starting LogStream Server v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let mut config = if args.config.exists() {
        ServerConfig::from_file(&args.config)?
    } else {
        info!("Config file not found, using defaults");
        ServerConfig::default()
    };

    // Override config with CLI arguments
    if let Some(socket) = args.socket {
        config.server.socket_path = socket;
    }
    if let Some(output) = args.output {
        config.storage.output_directory = output;
    }

    #[cfg(feature = "journald")]
    if args.journald {
        config.backends.journald.enabled = true;
    }

    #[cfg(feature = "metrics")]
    if args.metrics {
        config.metrics.enabled = true;
        config.metrics.port = args.metrics_port;
    }

    // Validate configuration
    config.validate()?;

    info!("Configuration loaded successfully");
    info!("Socket path: {}", config.server.socket_path);
    info!("Output directory: {}", config.storage.output_directory.display());
    info!("Max file size: {} bytes", config.storage.max_file_size);
    info!("Rotation enabled: {}", config.storage.rotation.enabled);

    // Initialize and start server
    let server = LogServer::new(config).await?;

    // Handle shutdown gracefully
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Shutdown signal received");
    };

    // Start server with graceful shutdown
    tokio::select! {
        result = server.start() => {
            if let Err(e) = result {
                error!("Server error: {}", e);
                std::process::exit(1);
            }
        }
        _ = shutdown_signal => {
            info!("Shutting down gracefully...");
        }
    }

    info!("LogStream Server stopped");
    Ok(())
}
