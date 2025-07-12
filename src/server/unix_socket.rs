//! Unix socket server implementation for LogStream

use crate::config::ServerConfig;
use crate::server::StorageBackend;
use crate::types::LogEntry;
use crate::{LogStreamError, Result};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;

/// Unix socket server for accepting log connections
pub struct UnixSocketServer {
    config: ServerConfig,
    storage: Arc<StorageBackend>,
    shutdown_rx: broadcast::Receiver<()>,
}

impl UnixSocketServer {
    /// Create a new Unix socket server
    pub async fn new(
        config: &ServerConfig,
        storage: Arc<StorageBackend>,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            storage,
            shutdown_rx,
        })
    }

    /// Start the Unix socket server
    pub async fn start(mut self) -> Result<()> {
        if Path::new(&self.config.server.socket_path).exists() {
            std::fs::remove_file(&self.config.server.socket_path)?;
        }

        let listener = UnixListener::bind(&self.config.server.socket_path)
            .map_err(|e| LogStreamError::Server(format!("Failed to bind socket: {}", e)))?;

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let storage = Arc::clone(&self.storage);
                            tokio::spawn(async move {
                                let _ = Self::handle_connection(stream, storage).await;
                            });
                        }
                        Err(e) => {
                            eprintln!("Failed to accept connection: {}", e);
                        }
                    }
                }
                _ = self.shutdown_rx.recv() => {
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_connection(
        stream: UnixStream,
        storage: Arc<StorageBackend>,
    ) -> Result<()> {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    if let Ok(entry) = serde_json::from_str::<LogEntry>(&line.trim()) {
                        storage.store_entry(entry).await?;
                    }
                }
                Err(_) => break,
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LogLevel;
    use std::path::Path;
    use tempfile::tempdir;
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;
    use tokio::time::{timeout, Duration};

    async fn create_test_server(socket_path: &str, output_dir: &Path) -> (UnixSocketServer, Arc<StorageBackend>, broadcast::Sender<()>) {
        let mut config = ServerConfig::default();
        config.server.socket_path = socket_path.to_string();
        config.storage.output_directory = output_dir.to_path_buf();
        config.backends.file.enabled = true;
        
        let storage = Arc::new(StorageBackend::new(&config).await.unwrap());
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        
        let server = UnixSocketServer::new(&config, storage.clone(), shutdown_rx).await.unwrap();
        
        (server, storage, shutdown_tx)
    }

    #[tokio::test]
    async fn test_unix_socket_server_creation() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("test.sock");
        let socket_str = socket_path.to_string_lossy().to_string();
        
        let (server, _, _) = create_test_server(&socket_str, temp_dir.path()).await;
        
        assert_eq!(server.config.server.socket_path, socket_str);
    }

    #[tokio::test]
    async fn test_server_removes_existing_socket() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("existing.sock");
        let socket_str = socket_path.to_string_lossy().to_string();
        
        // Create a file at the socket path
        std::fs::write(&socket_path, "dummy").unwrap();
        assert!(socket_path.exists());
        
        let (server, _, shutdown_tx) = create_test_server(&socket_str, temp_dir.path()).await;
        
        // Start server in background
        let server_handle = tokio::spawn(async move {
            server.start().await
        });
        
        // Give it time to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Socket file should have been removed and recreated
        assert!(socket_path.exists());
        
        // Shutdown
        let _ = shutdown_tx.send(());
        let _ = timeout(Duration::from_secs(1), server_handle).await;
    }

    #[tokio::test]
    async fn test_handle_connection() {
        let temp_dir = tempdir().unwrap();
        let mut config = ServerConfig::default();
        config.storage.output_directory = temp_dir.path().to_path_buf();
        config.backends.file.enabled = true;
        
        let storage = Arc::new(StorageBackend::new(&config).await.unwrap());
        
        // Create a pair of connected Unix sockets
        let (client, server) = UnixStream::pair().unwrap();
        
        // Handle connection in background
        let storage_clone = storage.clone();
        let handle = tokio::spawn(async move {
            UnixSocketServer::handle_connection(server, storage_clone).await
        });
        
        // Send a log entry
        let entry = LogEntry::new(
            LogLevel::Info,
            "test-daemon".to_string(),
            "Test message from handle_connection".to_string(),
        );
        
        let json = entry.to_json().unwrap();
        let mut client = client;
        client.write_all(json.as_bytes()).await.unwrap();
        client.write_all(b"\n").await.unwrap();
        client.flush().await.unwrap();
        
        // Close client to signal end
        drop(client);
        
        // Wait for handler to complete
        let result = timeout(Duration::from_secs(1), handle).await;
        assert!(result.is_ok());
        
        // Verify log was stored
        let log_file = temp_dir.path().join("test-daemon.log");
        assert!(log_file.exists());
        let content = tokio::fs::read_to_string(log_file).await.unwrap();
        assert!(content.contains("Test message from handle_connection"));
    }

    #[tokio::test]
    async fn test_server_accepts_multiple_connections() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("multi.sock");
        let socket_str = socket_path.to_string_lossy().to_string();
        
        let (server, _storage, shutdown_tx) = create_test_server(&socket_str, temp_dir.path()).await;
        
        // Start server
        let server_handle = tokio::spawn(async move {
            server.start().await
        });
        
        // Wait for server to start
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Connect multiple clients
        for i in 0..3 {
            let socket_path = socket_str.clone();
            tokio::spawn(async move {
                if let Ok(mut stream) = UnixStream::connect(&socket_path).await {
                    let entry = LogEntry::new(
                        LogLevel::Info,
                        format!("client-{}", i),
                        format!("Message from client {}", i),
                    );
                    let json = entry.to_json().unwrap();
                    let _ = stream.write_all(json.as_bytes()).await;
                    let _ = stream.write_all(b"\n").await;
                    let _ = stream.flush().await;
                }
            });
        }
        
        // Give time for messages to be processed
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Shutdown
        let _ = shutdown_tx.send(());
        let _ = timeout(Duration::from_secs(1), server_handle).await;
        
        // Verify multiple log files were created
        for i in 0..3 {
            let log_file = temp_dir.path().join(format!("client-{}.log", i));
            assert!(log_file.exists(), "Log file for client-{} should exist", i);
        }
    }

    #[tokio::test]
    async fn test_server_handles_invalid_json() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("invalid.sock");
        let socket_str = socket_path.to_string_lossy().to_string();
        
        let (server, _, shutdown_tx) = create_test_server(&socket_str, temp_dir.path()).await;
        
        // Start server
        let server_handle = tokio::spawn(async move {
            server.start().await
        });
        
        // Wait for server to start
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Send invalid JSON
        let mut stream = UnixStream::connect(&socket_str).await.unwrap();
        stream.write_all(b"invalid json\n").await.unwrap();
        stream.write_all(b"{broken: json\n").await.unwrap();
        
        // Send valid JSON after invalid
        let entry = LogEntry::new(
            LogLevel::Info,
            "valid-daemon".to_string(),
            "Valid message after invalid".to_string(),
        );
        let json = entry.to_json().unwrap();
        stream.write_all(json.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        stream.flush().await.unwrap();
        
        // Give time to process
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Shutdown
        let _ = shutdown_tx.send(());
        let _ = timeout(Duration::from_secs(1), server_handle).await;
        
        // Only valid entry should be stored
        let valid_log = temp_dir.path().join("valid-daemon.log");
        assert!(valid_log.exists());
        let content = tokio::fs::read_to_string(valid_log).await.unwrap();
        assert!(content.contains("Valid message after invalid"));
    }

    #[tokio::test]
    async fn test_server_shutdown_response() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("shutdown.sock");
        let socket_str = socket_path.to_string_lossy().to_string();
        
        let (server, _, shutdown_tx) = create_test_server(&socket_str, temp_dir.path()).await;
        
        // Start server
        let server_handle = tokio::spawn(async move {
            server.start().await
        });
        
        // Wait for server to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Send shutdown signal
        let _ = shutdown_tx.send(());
        
        // Server should shutdown cleanly
        let result = timeout(Duration::from_secs(2), server_handle).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_client_connections() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("concurrent.sock");
        let socket_str = socket_path.to_string_lossy().to_string();
        
        let (server, _, shutdown_tx) = create_test_server(&socket_str, temp_dir.path()).await;
        
        // Start server
        let server_handle = tokio::spawn(async move {
            server.start().await
        });
        
        // Wait for server to start
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Connect many clients concurrently
        let mut handles = vec![];
        for i in 0..10 {
            let socket_path = socket_str.clone();
            let handle = tokio::spawn(async move {
                if let Ok(mut stream) = UnixStream::connect(&socket_path).await {
                    for j in 0..5 {
                        let entry = LogEntry::new(
                            LogLevel::Info,
                            "concurrent-daemon".to_string(),
                            format!("Message {} from client {}", j, i),
                        );
                        let json = entry.to_json().unwrap();
                        let _ = stream.write_all(json.as_bytes()).await;
                        let _ = stream.write_all(b"\n").await;
                    }
                    let _ = stream.flush().await;
                }
            });
            handles.push(handle);
        }
        
        // Wait for all clients
        for handle in handles {
            let _ = handle.await;
        }
        
        // Give time to process all messages
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Shutdown
        let _ = shutdown_tx.send(());
        let _ = timeout(Duration::from_secs(1), server_handle).await;
        
        // Verify all messages were stored
        let log_file = temp_dir.path().join("concurrent-daemon.log");
        assert!(log_file.exists());
        let content = tokio::fs::read_to_string(log_file).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 50); // 10 clients * 5 messages each
    }
}
