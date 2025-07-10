//! LogStream Client Example

use logstream::client::LogClient;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting LogStream Client Example");

    let socket_path = "/tmp/logstream.sock";
    let daemon_name = "example-daemon";

    match LogClient::connect(socket_path, daemon_name).await {
        Ok(client) => {
            println!("Connected to LogStream server");
            
            // Simple logging
            client.info("Application started").await?;
            
            // Logging with structured fields
            let mut fields = HashMap::new();
            fields.insert("user_id".to_string(), "12345".to_string());
            fields.insert("action".to_string(), "login".to_string());
            
            client.info_with_fields("User login successful", fields).await?;
            
            // Simulate some activity
            for i in 1..=5 {
                client.info(&format!("Processing iteration {}", i)).await?;
                sleep(Duration::from_millis(500)).await;
            }
            
            client.close().await?;
            println!("Client connection closed");
        }
        Err(e) => {
            eprintln!("Failed to connect: {}", e);
            eprintln!("Make sure the server is running");
        }
    }

    Ok(())
}