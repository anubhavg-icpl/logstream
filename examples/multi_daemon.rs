//! Multi-Daemon LogStream Example

use logstream::client::LogClient;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use rand::Rng;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Multi-Daemon LogStream Example");

    let socket_path = "/tmp/logstream.sock";

    // Define daemon configurations
    let daemon_configs = vec![
        ("web-server", 1000),
        ("auth-service", 1500),
        ("database", 2000),
        ("cache-manager", 5000),
    ];

    println!("Starting {} daemons", daemon_configs.len());

    // Start each daemon in its own task
    let mut handles = Vec::new();
    
    for (daemon_name, interval_ms) in daemon_configs {
        let socket_path = socket_path.to_string();
        let handle = tokio::spawn(async move {
            if let Err(e) = run_daemon(socket_path, daemon_name, interval_ms).await {
                eprintln!("Daemon {} error: {}", daemon_name, e);
            }
        });
        handles.push(handle);
    }

    // Let the daemons run for 30 seconds
    println!("Daemons are running. Monitoring for 30 seconds...");
    sleep(Duration::from_secs(30)).await;

    println!("Stopping all daemons...");
    for handle in handles {
        handle.abort();
    }

    println!("Multi-daemon example completed");
    Ok(())
}

async fn run_daemon(socket_path: String, daemon_name: &str, interval_ms: u64) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting daemon: {}", daemon_name);

    let client = LogClient::connect(&socket_path, daemon_name).await?;
    let mut rng = rand::thread_rng();

    // Send startup message
    let mut startup_fields = HashMap::new();
    startup_fields.insert("version".to_string(), "1.0.0".to_string());
    startup_fields.insert("pid".to_string(), std::process::id().to_string());
    
    client.info_with_fields(&format!("{} daemon started", daemon_name), startup_fields).await?;

    loop {
        sleep(Duration::from_millis(interval_ms)).await;

        let mut fields = HashMap::new();
        fields.insert("component".to_string(), daemon_name.to_string());

        // Simulate different types of messages
        match rng.gen_range(0..10) {
            0..=6 => {
                fields.insert("status".to_string(), "ok".to_string());
                client.info_with_fields("Regular operation", fields).await?;
            }
            7..=8 => {
                fields.insert("status".to_string(), "warning".to_string());
                client.warning("Minor issue detected", fields).await?;
            }
            9 => {
                fields.insert("status".to_string(), "error".to_string());
                client.error("Error occurred", fields).await?;
            }
            _ => unreachable!(),
        }
    }
}