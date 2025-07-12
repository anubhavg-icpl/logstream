//! Integration tests for LogStream

use logstream::client::LogClient;
use logstream::config::ServerConfig;
use logstream::server::LogServer;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::tempdir;
use tokio::fs;
use tokio::time::{sleep, timeout};

/// Helper function to create a test server config
async fn create_test_server_config(socket_path: &str, log_dir: &PathBuf) -> ServerConfig {
    let mut config = ServerConfig::default();
    config.server.socket_path = socket_path.to_string();
    config.storage.output_directory = log_dir.clone();
    config.backends.file.enabled = true;
    config.backends.file.format = "json".to_string();
    config
}

/// Test basic client-server communication
#[tokio::test]
async fn test_basic_logging() {
    let temp_dir = tempdir().unwrap();
    let socket_path = temp_dir.path().join("test.sock");
    let socket_str = socket_path.to_string_lossy().to_string();
    let log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&log_dir).await.unwrap();
    
    // Create and start server
    let config = create_test_server_config(&socket_str, &log_dir).await;
    let server = LogServer::new(config).await.unwrap();
    
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Give server time to start
    sleep(Duration::from_millis(200)).await;
    
    // Connect client and send logs
    let client = LogClient::connect(&socket_str, "test-daemon").await.unwrap();
    
    // Send various log levels
    client.emergency("Emergency message").await.unwrap();
    client.alert("Alert message").await.unwrap();
    client.critical("Critical message").await.unwrap();
    client.error("Error message").await.unwrap();
    client.warning("Warning message").await.unwrap();
    client.notice("Notice message").await.unwrap();
    client.info("Info message").await.unwrap();
    client.debug("Debug message").await.unwrap();
    
    // Close client
    client.close().await.unwrap();
    
    // Give time for logs to be written
    sleep(Duration::from_millis(100)).await;
    
    // Verify logs were written
    let log_file = log_dir.join("test-daemon.log");
    assert!(log_file.exists());
    
    let content = fs::read_to_string(log_file).await.unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 8);
    
    // Verify all log levels are present
    assert!(content.contains("Emergency"));
    assert!(content.contains("Alert"));
    assert!(content.contains("Critical"));
    assert!(content.contains("Error"));
    assert!(content.contains("Warning"));
    assert!(content.contains("Notice"));
    assert!(content.contains("Info"));
    assert!(content.contains("Debug"));
    
    // Shutdown server
    server_handle.abort();
}

/// Test multiple clients connecting to the same server
#[tokio::test]
async fn test_multiple_clients() {
    let temp_dir = tempdir().unwrap();
    let socket_path = temp_dir.path().join("multi_client.sock");
    let socket_str = socket_path.to_string_lossy().to_string();
    let log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&log_dir).await.unwrap();
    
    // Create and start server
    let config = create_test_server_config(&socket_str, &log_dir).await;
    let server = LogServer::new(config).await.unwrap();
    
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Give server time to start
    sleep(Duration::from_millis(200)).await;
    
    // Connect multiple clients
    let mut client_handles = vec![];
    
    for i in 0..5 {
        let socket_path = socket_str.clone();
        let daemon_name = format!("daemon-{}", i);
        
        let handle = tokio::spawn(async move {
            let client = LogClient::connect(&socket_path, &daemon_name).await.unwrap();
            
            // Each client sends 10 messages
            for j in 0..10 {
                client.info(&format!("Message {} from {}", j, daemon_name)).await.unwrap();
            }
            
            client.close().await.unwrap();
        });
        
        client_handles.push(handle);
    }
    
    // Wait for all clients to finish
    for handle in client_handles {
        handle.await.unwrap();
    }
    
    // Give time for logs to be written
    sleep(Duration::from_millis(200)).await;
    
    // Verify logs from all daemons
    for i in 0..5 {
        let log_file = log_dir.join(format!("daemon-{}.log", i));
        assert!(log_file.exists());
        
        let content = fs::read_to_string(log_file).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 10);
        
        // Verify all messages are present
        for j in 0..10 {
            assert!(content.contains(&format!("Message {} from daemon-{}", j, i)));
        }
    }
    
    // Shutdown server
    server_handle.abort();
}

/// Test logging with structured fields
#[tokio::test]
async fn test_structured_logging() {
    let temp_dir = tempdir().unwrap();
    let socket_path = temp_dir.path().join("structured.sock");
    let socket_str = socket_path.to_string_lossy().to_string();
    let log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&log_dir).await.unwrap();
    
    // Create and start server
    let config = create_test_server_config(&socket_str, &log_dir).await;
    let server = LogServer::new(config).await.unwrap();
    
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Give server time to start
    sleep(Duration::from_millis(200)).await;
    
    // Connect client
    let client = LogClient::connect(&socket_str, "structured-daemon").await.unwrap();
    
    // Create structured fields
    let mut fields = HashMap::new();
    fields.insert("user_id".to_string(), "12345".to_string());
    fields.insert("request_id".to_string(), "req-abcdef".to_string());
    fields.insert("ip_address".to_string(), "192.168.1.100".to_string());
    fields.insert("action".to_string(), "login".to_string());
    
    // Send logs with fields
    client.info_with_fields("User login successful", fields.clone()).await.unwrap();
    
    // Send error with different fields
    let mut error_fields = HashMap::new();
    error_fields.insert("error_code".to_string(), "AUTH_FAILED".to_string());
    error_fields.insert("attempts".to_string(), "3".to_string());
    
    client.error_with_fields("Authentication failed", error_fields).await.unwrap();
    
    // Close client
    client.close().await.unwrap();
    
    // Give time for logs to be written
    sleep(Duration::from_millis(100)).await;
    
    // Verify structured logs
    let log_file = log_dir.join("structured-daemon.log");
    assert!(log_file.exists());
    
    let content = fs::read_to_string(log_file).await.unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);
    
    // Parse and verify first log entry
    let first_entry: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first_entry["level"], "Info");
    assert_eq!(first_entry["message"], "User login successful");
    assert_eq!(first_entry["fields"]["user_id"], "12345");
    assert_eq!(first_entry["fields"]["request_id"], "req-abcdef");
    assert_eq!(first_entry["fields"]["ip_address"], "192.168.1.100");
    assert_eq!(first_entry["fields"]["action"], "login");
    
    // Parse and verify second log entry
    let second_entry: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(second_entry["level"], "Error");
    assert_eq!(second_entry["message"], "Authentication failed");
    assert_eq!(second_entry["fields"]["error_code"], "AUTH_FAILED");
    assert_eq!(second_entry["fields"]["attempts"], "3");
    
    // Shutdown server
    server_handle.abort();
}

/// Test high-throughput logging
#[tokio::test]
async fn test_high_throughput() {
    let temp_dir = tempdir().unwrap();
    let socket_path = temp_dir.path().join("throughput.sock");
    let socket_str = socket_path.to_string_lossy().to_string();
    let log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&log_dir).await.unwrap();
    
    // Create and start server
    let config = create_test_server_config(&socket_str, &log_dir).await;
    let server = LogServer::new(config).await.unwrap();
    
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Give server time to start
    sleep(Duration::from_millis(200)).await;
    
    // Connect client
    let client = LogClient::connect(&socket_str, "throughput-daemon").await.unwrap();
    
    let start = std::time::Instant::now();
    
    // Send many messages rapidly
    for i in 0..1000 {
        client.info(&format!("High throughput message {}", i)).await.unwrap();
    }
    
    let elapsed = start.elapsed();
    println!("Sent 1000 messages in {:?}", elapsed);
    
    // Close client
    client.close().await.unwrap();
    
    // Give time for logs to be written
    sleep(Duration::from_millis(500)).await;
    
    // Verify all messages were logged
    let log_file = log_dir.join("throughput-daemon.log");
    assert!(log_file.exists());
    
    let content = fs::read_to_string(log_file).await.unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1000);
    
    // Verify some sample messages
    assert!(content.contains("High throughput message 0"));
    assert!(content.contains("High throughput message 500"));
    assert!(content.contains("High throughput message 999"));
    
    // Shutdown server
    server_handle.abort();
}

/// Test client reconnection after server restart
#[tokio::test]
async fn test_client_reconnection() {
    let temp_dir = tempdir().unwrap();
    let socket_path = temp_dir.path().join("reconnect.sock");
    let socket_str = socket_path.to_string_lossy().to_string();
    let log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&log_dir).await.unwrap();
    
    // Create and start first server instance
    let config = create_test_server_config(&socket_str, &log_dir).await;
    let server = LogServer::new(config.clone()).await.unwrap();
    
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Give server time to start
    sleep(Duration::from_millis(200)).await;
    
    // Connect client and send initial message
    let client = LogClient::connect(&socket_str, "reconnect-daemon").await.unwrap();
    client.info("Message before server restart").await.unwrap();
    
    // Give time for the log to be written
    sleep(Duration::from_millis(100)).await;
    
    // Shutdown server
    server_handle.abort();
    sleep(Duration::from_millis(100)).await;
    
    // Start new server instance
    let server2 = LogServer::new(config).await.unwrap();
    let server_handle2 = tokio::spawn(async move {
        server2.start().await
    });
    
    // Give new server time to start
    sleep(Duration::from_millis(200)).await;
    
    // Client should reconnect and send more messages
    client.info("Message after server restart").await.unwrap();
    client.info("Another message after restart").await.unwrap();
    
    // Close client
    client.close().await.unwrap();
    
    // Give time for logs to be written
    sleep(Duration::from_millis(100)).await;
    
    // Verify all messages were logged
    let log_file = log_dir.join("reconnect-daemon.log");
    assert!(log_file.exists());
    
    let content = fs::read_to_string(&log_file).await.unwrap();
    assert!(content.contains("Message before server restart"));
    assert!(content.contains("Message after server restart"));
    assert!(content.contains("Another message after restart"));
    
    // Shutdown second server
    server_handle2.abort();
}

/// Test concurrent clients with mixed operations
#[tokio::test]
async fn test_concurrent_mixed_operations() {
    let temp_dir = tempdir().unwrap();
    let socket_path = temp_dir.path().join("concurrent.sock");
    let socket_str = socket_path.to_string_lossy().to_string();
    let log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&log_dir).await.unwrap();
    
    // Create and start server
    let config = create_test_server_config(&socket_str, &log_dir).await;
    let server = LogServer::new(config).await.unwrap();
    
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Give server time to start
    sleep(Duration::from_millis(200)).await;
    
    // Launch multiple clients doing different things
    let mut handles = vec![];
    
    // Client 1: Rapid fire logging
    let socket_path1 = socket_str.clone();
    handles.push(tokio::spawn(async move {
        let client = LogClient::connect(&socket_path1, "rapid-fire").await.unwrap();
        for i in 0..100 {
            client.info(&format!("Rapid message {}", i)).await.unwrap();
        }
        client.close().await.unwrap();
    }));
    
    // Client 2: Mixed log levels
    let socket_path2 = socket_str.clone();
    handles.push(tokio::spawn(async move {
        let client = LogClient::connect(&socket_path2, "mixed-levels").await.unwrap();
        for i in 0..20 {
            match i % 4 {
                0 => client.debug(&format!("Debug {}", i)).await.unwrap(),
                1 => client.info(&format!("Info {}", i)).await.unwrap(),
                2 => client.warning(&format!("Warning {}", i)).await.unwrap(),
                3 => client.error(&format!("Error {}", i)).await.unwrap(),
                _ => unreachable!(),
            }
        }
        client.close().await.unwrap();
    }));
    
    // Client 3: Structured logging
    let socket_path3 = socket_str.clone();
    handles.push(tokio::spawn(async move {
        let client = LogClient::connect(&socket_path3, "structured").await.unwrap();
        for i in 0..30 {
            let mut fields = HashMap::new();
            fields.insert("iteration".to_string(), i.to_string());
            fields.insert("thread".to_string(), "client-3".to_string());
            client.info_with_fields(&format!("Structured message {}", i), fields).await.unwrap();
        }
        client.close().await.unwrap();
    }));
    
    // Wait for all clients
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Give time for logs to be written
    sleep(Duration::from_millis(200)).await;
    
    // Verify logs from all clients
    let rapid_log = log_dir.join("rapid-fire.log");
    assert!(rapid_log.exists());
    let rapid_content = fs::read_to_string(rapid_log).await.unwrap();
    assert_eq!(rapid_content.lines().count(), 100);
    
    let mixed_log = log_dir.join("mixed-levels.log");
    assert!(mixed_log.exists());
    let mixed_content = fs::read_to_string(mixed_log).await.unwrap();
    assert_eq!(mixed_content.lines().count(), 20);
    
    let structured_log = log_dir.join("structured.log");
    assert!(structured_log.exists());
    let structured_content = fs::read_to_string(structured_log).await.unwrap();
    assert_eq!(structured_content.lines().count(), 30);
    
    // Shutdown server
    server_handle.abort();
}

/// Test error handling and edge cases
#[tokio::test]
async fn test_error_handling() {
    let temp_dir = tempdir().unwrap();
    let socket_path = temp_dir.path().join("error_test.sock");
    let socket_str = socket_path.to_string_lossy().to_string();
    
    // Try to connect to non-existent server
    let result = timeout(
        Duration::from_secs(2),
        LogClient::connect(&socket_str, "test-daemon")
    ).await;
    
    assert!(result.is_ok()); // Timeout didn't expire
    assert!(result.unwrap().is_err()); // Connection failed
    
    // Test empty daemon name (should fail validation)
    let result = LogClient::connect(&socket_str, "").await;
    assert!(result.is_err());
}

/// Test log entry metadata
#[tokio::test]
async fn test_log_metadata() {
    let temp_dir = tempdir().unwrap();
    let socket_path = temp_dir.path().join("metadata.sock");
    let socket_str = socket_path.to_string_lossy().to_string();
    let log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&log_dir).await.unwrap();
    
    // Create and start server
    let config = create_test_server_config(&socket_str, &log_dir).await;
    let server = LogServer::new(config).await.unwrap();
    
    let server_handle = tokio::spawn(async move {
        server.start().await
    });
    
    // Give server time to start
    sleep(Duration::from_millis(200)).await;
    
    // Connect client
    let client = LogClient::connect(&socket_str, "metadata-daemon").await.unwrap();
    
    // Send a log message
    client.info("Test metadata message").await.unwrap();
    
    // Close client
    client.close().await.unwrap();
    
    // Give time for logs to be written
    sleep(Duration::from_millis(100)).await;
    
    // Read and parse log entry
    let log_file = log_dir.join("metadata-daemon.log");
    let content = fs::read_to_string(log_file).await.unwrap();
    let entry: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    
    // Verify metadata fields
    assert!(entry["id"].is_string());
    assert!(!entry["id"].as_str().unwrap().is_empty());
    
    assert!(entry["timestamp"].is_string());
    assert!(!entry["timestamp"].as_str().unwrap().is_empty());
    
    assert_eq!(entry["level"], "Info");
    assert_eq!(entry["daemon"], "metadata-daemon");
    assert_eq!(entry["message"], "Test metadata message");
    
    assert!(entry["pid"].is_number());
    assert!(entry["pid"].as_u64().unwrap() > 0);
    
    assert!(entry["hostname"].is_string());
    assert!(!entry["hostname"].as_str().unwrap().is_empty());
    
    // Shutdown server
    server_handle.abort();
}