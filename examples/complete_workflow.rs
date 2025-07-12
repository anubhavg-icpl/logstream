//! Complete LogStream Workflow Example
//! 
//! This example demonstrates a complete end-to-end workflow including:
//! - Setting up a LogStream server with custom configuration
//! - Connecting multiple clients
//! - Sending structured logs with fields
//! - Demonstrating all log levels
//! - Showing log rotation behavior
//! - Graceful shutdown

use logstream::client::LogClient;
use logstream::config::ServerConfig;
use logstream::server::LogServer;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tokio::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== LogStream Complete Workflow Example ===\n");

    // 1. Setup directories
    let base_dir = PathBuf::from("/tmp/logstream-example");
    let log_dir = base_dir.join("logs");
    fs::create_dir_all(&log_dir).await?;
    println!("✓ Created log directory: {}", log_dir.display());

    // 2. Configure server
    let mut config = ServerConfig::default();
    config.server.socket_path = base_dir.join("logstream.sock").to_string_lossy().to_string();
    config.server.max_connections = 100;
    config.server.buffer_size = 8192;
    
    config.storage.output_directory = log_dir.clone();
    config.storage.max_file_size = 10 * 1024 * 1024; // 10MB
    config.storage.rotation.enabled = true;
    config.storage.rotation.max_age_hours = 24;
    config.storage.rotation.keep_files = 7;
    
    config.backends.file.enabled = true;
    config.backends.file.format = "json".to_string();
    
    println!("✓ Server configuration:");
    println!("  - Socket: {}", config.server.socket_path);
    println!("  - Log directory: {}", config.storage.output_directory.display());
    println!("  - Max file size: {} MB", config.storage.max_file_size / 1024 / 1024);
    println!("  - Rotation enabled: {}", config.storage.rotation.enabled);
    println!();

    // 3. Start server
    let server = LogServer::new(config.clone()).await?;
    let server_handle = tokio::spawn(async move {
        println!("✓ Starting LogStream server...");
        if let Err(e) = server.start().await {
            eprintln!("Server error: {}", e);
        }
    });

    // Give server time to start
    sleep(Duration::from_millis(500)).await;
    println!("✓ Server is running\n");

    // 4. Simulate different applications/daemons
    println!("=== Simulating Application Logs ===\n");

    // Web Server logs
    let web_client = LogClient::connect(&config.server.socket_path, "web-server").await?;
    println!("✓ Web server connected");

    // Log startup
    let mut startup_fields = HashMap::new();
    startup_fields.insert("version".to_string(), "2.1.0".to_string());
    startup_fields.insert("port".to_string(), "8080".to_string());
    startup_fields.insert("workers".to_string(), "4".to_string());
    web_client.info_with_fields("Web server started", startup_fields).await?;

    // Simulate web requests
    for i in 0..5 {
        let mut fields = HashMap::new();
        fields.insert("method".to_string(), "GET".to_string());
        fields.insert("path".to_string(), format!("/api/users/{}", i));
        fields.insert("status".to_string(), "200".to_string());
        fields.insert("duration_ms".to_string(), format!("{}", 50 + i * 10));
        fields.insert("ip".to_string(), format!("192.168.1.{}", 100 + i));
        
        web_client.info_with_fields("HTTP request processed", fields).await?;
        sleep(Duration::from_millis(100)).await;
    }

    // Database Service logs
    let db_client = LogClient::connect(&config.server.socket_path, "database-service").await?;
    println!("✓ Database service connected");

    // Normal operations
    db_client.info("Database connection pool initialized").await?;
    db_client.debug("Connection pool size: 10").await?;

    // Simulate a warning
    let mut warn_fields = HashMap::new();
    warn_fields.insert("query_time_ms".to_string(), "5234".to_string());
    warn_fields.insert("query".to_string(), "SELECT * FROM large_table".to_string());
    db_client.warning_with_fields("Slow query detected", warn_fields).await?;

    // Simulate an error
    let mut error_fields = HashMap::new();
    error_fields.insert("error_code".to_string(), "CONNECTION_TIMEOUT".to_string());
    error_fields.insert("retry_count".to_string(), "3".to_string());
    error_fields.insert("database".to_string(), "analytics".to_string());
    db_client.error_with_fields("Failed to connect to replica", error_fields).await?;

    // Authentication Service logs
    let auth_client = LogClient::connect(&config.server.socket_path, "auth-service").await?;
    println!("✓ Authentication service connected");

    // Successful login
    let mut login_fields = HashMap::new();
    login_fields.insert("user_id".to_string(), "user123".to_string());
    login_fields.insert("method".to_string(), "password".to_string());
    login_fields.insert("ip".to_string(), "203.0.113.45".to_string());
    auth_client.info_with_fields("User login successful", login_fields).await?;

    // Failed login attempt
    let mut failed_fields = HashMap::new();
    failed_fields.insert("username".to_string(), "admin".to_string());
    failed_fields.insert("ip".to_string(), "198.51.100.14".to_string());
    failed_fields.insert("reason".to_string(), "invalid_password".to_string());
    auth_client.warning_with_fields("Login attempt failed", failed_fields).await?;

    // Critical security event
    let mut critical_fields = HashMap::new();
    critical_fields.insert("ip".to_string(), "192.0.2.78".to_string());
    critical_fields.insert("attempts".to_string(), "50".to_string());
    critical_fields.insert("timeframe".to_string(), "60s".to_string());
    auth_client.critical_with_fields("Brute force attack detected", critical_fields).await?;

    // 5. Demonstrate all log levels
    println!("\n=== Demonstrating All Log Levels ===");
    
    let system_client = LogClient::connect(&config.server.socket_path, "system-monitor").await?;
    
    system_client.debug("Debug: Checking system resources").await?;
    system_client.info("Info: System health check completed").await?;
    system_client.notice("Notice: Scheduled maintenance window approaching").await?;
    system_client.warning("Warning: Disk usage at 80%").await?;
    system_client.error("Error: Failed to backup configuration").await?;
    system_client.critical("Critical: Primary database unreachable").await?;
    system_client.alert("Alert: Security breach detected").await?;
    system_client.emergency("Emergency: System shutdown imminent").await?;

    println!("✓ All log levels demonstrated\n");

    // 6. Generate some volume for rotation testing
    println!("=== Generating Log Volume ===");
    let volume_client = LogClient::connect(&config.server.socket_path, "load-generator").await?;
    
    for batch in 0..5 {
        println!("  Generating batch {}...", batch + 1);
        for i in 0..100 {
            let mut fields = HashMap::new();
            fields.insert("batch".to_string(), batch.to_string());
            fields.insert("sequence".to_string(), i.to_string());
            fields.insert("data".to_string(), "x".repeat(100)); // Add some bulk
            
            volume_client.info_with_fields("Bulk log entry", fields).await?;
        }
    }
    println!("✓ Generated 500 log entries\n");

    // 7. Close all clients
    println!("=== Cleanup ===");
    web_client.close().await?;
    println!("✓ Web server client closed");
    
    db_client.close().await?;
    println!("✓ Database client closed");
    
    auth_client.close().await?;
    println!("✓ Auth service client closed");
    
    system_client.close().await?;
    println!("✓ System monitor client closed");
    
    volume_client.close().await?;
    println!("✓ Load generator client closed");

    // Give time for final logs to be written
    sleep(Duration::from_millis(500)).await;

    // 8. Show results
    println!("\n=== Results ===");
    
    // List generated log files
    let mut entries = fs::read_dir(&log_dir).await?;
    let mut log_files = Vec::new();
    
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("log") {
            log_files.push(path);
        }
    }
    
    log_files.sort();
    println!("Generated {} log files:", log_files.len());
    
    for file in &log_files {
        let metadata = fs::metadata(file).await?;
        println!("  - {} ({} KB)", 
            file.file_name().unwrap().to_string_lossy(),
            metadata.len() / 1024
        );
    }

    // Sample some log entries
    if let Some(first_file) = log_files.first() {
        println!("\nSample entries from {}:", first_file.file_name().unwrap().to_string_lossy());
        let content = fs::read_to_string(first_file).await?;
        let lines: Vec<&str> = content.lines().take(3).collect();
        
        for (i, line) in lines.iter().enumerate() {
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                println!("\n  Entry {}:", i + 1);
                println!("    Level: {}", entry["level"]);
                println!("    Daemon: {}", entry["daemon"]);
                println!("    Message: {}", entry["message"]);
                if let Some(fields) = entry["fields"].as_object() {
                    if !fields.is_empty() {
                        println!("    Fields:");
                        for (key, value) in fields {
                            println!("      - {}: {}", key, value);
                        }
                    }
                }
            }
        }
    }

    // 9. Shutdown server
    println!("\n✓ Shutting down server...");
    server_handle.abort();
    
    println!("\n=== Example Complete ===");
    println!("Log files are available at: {}", log_dir.display());
    
    Ok(())
}