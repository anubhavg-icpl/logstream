//! LogStream Server Example

use logstream::config::{ServerConfig, ServerSettings, StorageSettings, RotationSettings};
use logstream::server::LogServer;
use std::path::PathBuf;
use tempfile::tempdir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting LogStream Server Example");

    // Create temporary directory for this example
    let temp_dir = tempdir()?;
    let socket_path = temp_dir.path().join("logstream_example.sock");
    let log_dir = temp_dir.path().join("logs");
    std::fs::create_dir_all(&log_dir)?;

    // Create custom configuration
    let mut config = ServerConfig::default();
    config.server.socket_path = socket_path.to_string_lossy().to_string();
    config.storage.output_directory = log_dir;
    config.storage.max_file_size = 10 * 1024 * 1024; // 10MB for demo

    println!("Configuration created:");
    println!("  Socket: {}", config.server.socket_path);
    println!("  Log directory: {}", config.storage.output_directory.display());

    // Create and start server
    let server = LogServer::new(config).await?;
    
    println!("LogStream server starting...");
    println!("Press Ctrl+C to stop the server");

    // Start server (this will run until interrupted)
    if let Err(e) = server.start().await {
        eprintln!("Server error: {}", e);
    }

    println!("Server stopped");
    Ok(())
}