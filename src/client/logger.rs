//! LogStream client implementation for sending logs to the centralized server

use crate::config::ClientConfig;
use crate::types::{LogEntry, LogFields, LogLevel};
use crate::{LogStreamError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

/// High-performance client for sending logs to LogStream server
#[derive(Clone)]
pub struct LogClient {
    config: ClientConfig,
    connection: Arc<Mutex<Option<UnixStream>>>,
    hostname: String,
}

impl LogClient {
    /// Create a new log client connecting to specified socket path
    pub async fn connect(socket_path: &str, daemon_name: &str) -> Result<Self> {
        let config = ClientConfig {
            socket_path: socket_path.to_string(),
            daemon_name: daemon_name.to_string(),
            ..Default::default()
        };
        
        Self::with_config(config).await
    }

    /// Create a new log client with custom configuration
    pub async fn with_config(config: ClientConfig) -> Result<Self> {
        config.validate()?;
        
        let hostname = gethostname::gethostname()
            .to_string_lossy()
            .to_string();

        let client = Self {
            config,
            connection: Arc::new(Mutex::new(None)),
            hostname,
        };

        client.ensure_connected().await?;
        Ok(client)
    }

    /// Ensure we have an active connection to the server
    async fn ensure_connected(&self) -> Result<()> {
        let mut conn_guard = self.connection.lock().await;
        
        if conn_guard.is_none() {
            let connect_future = UnixStream::connect(&self.config.socket_path);
            let conn = timeout(Duration::from_secs(self.config.timeout_seconds), connect_future)
                .await
                .map_err(|_| LogStreamError::Connection("Connection timeout".to_string()))?
                .map_err(|e| LogStreamError::Connection(format!("Failed to connect: {}", e)))?;

            *conn_guard = Some(conn);
        }
        
        Ok(())
    }

    /// Log an info message
    pub async fn info<S: AsRef<str>>(&self, message: S) -> Result<()> {
        self.log(LogLevel::Info, message.as_ref(), HashMap::new()).await
    }

    /// Log an info message with fields
    pub async fn info_with_fields<S: AsRef<str>>(&self, message: S, fields: LogFields) -> Result<()> {
        self.log(LogLevel::Info, message.as_ref(), fields).await
    }

    /// Log a message with specified level and fields
    pub async fn log(&self, level: LogLevel, message: &str, fields: LogFields) -> Result<()> {
        let mut entry = LogEntry::new(level, self.config.daemon_name.clone(), message.to_string());
        entry.fields = fields;
        entry.pid = Some(std::process::id());
        entry.hostname = Some(self.hostname.clone());

        let json_data = entry.to_json()?;
        let message = format!("{}\n", json_data);

        self.ensure_connected().await?;
        
        let mut conn_guard = self.connection.lock().await;
        if let Some(ref mut conn) = *conn_guard {
            conn.write_all(message.as_bytes()).await?;
            conn.flush().await?;
        }

        Ok(())
    }

    /// Log an emergency message
    pub async fn emergency<S: AsRef<str>>(&self, message: S) -> Result<()> {
        self.log(LogLevel::Emergency, message.as_ref(), HashMap::new()).await
    }

    /// Log an alert message
    pub async fn alert<S: AsRef<str>>(&self, message: S) -> Result<()> {
        self.log(LogLevel::Alert, message.as_ref(), HashMap::new()).await
    }

    /// Log a critical message
    pub async fn critical<S: AsRef<str>>(&self, message: S) -> Result<()> {
        self.log(LogLevel::Critical, message.as_ref(), HashMap::new()).await
    }

    /// Log a critical message with fields
    pub async fn critical_with_fields<S: AsRef<str>>(&self, message: S, fields: LogFields) -> Result<()> {
        self.log(LogLevel::Critical, message.as_ref(), fields).await
    }

    /// Log an error message
    pub async fn error<S: AsRef<str>>(&self, message: S) -> Result<()> {
        self.log(LogLevel::Error, message.as_ref(), HashMap::new()).await
    }

    /// Log a warning message
    pub async fn warning<S: AsRef<str>>(&self, message: S) -> Result<()> {
        self.log(LogLevel::Warning, message.as_ref(), HashMap::new()).await
    }

    /// Log a warning message with fields
    pub async fn warning_with_fields<S: AsRef<str>>(&self, message: S, fields: LogFields) -> Result<()> {
        self.log(LogLevel::Warning, message.as_ref(), fields).await
    }

    /// Log an error message with fields
    pub async fn error_with_fields<S: AsRef<str>>(&self, message: S, fields: LogFields) -> Result<()> {
        self.log(LogLevel::Error, message.as_ref(), fields).await
    }

    /// Log a notice message
    pub async fn notice<S: AsRef<str>>(&self, message: S) -> Result<()> {
        self.log(LogLevel::Notice, message.as_ref(), HashMap::new()).await
    }

    /// Log a debug message
    pub async fn debug<S: AsRef<str>>(&self, message: S) -> Result<()> {
        self.log(LogLevel::Debug, message.as_ref(), HashMap::new()).await
    }

    /// Close the connection to the server
    pub async fn close(&self) -> Result<()> {
        let mut conn_guard = self.connection.lock().await;
        if let Some(mut conn) = conn_guard.take() {
            conn.shutdown().await.map_err(LogStreamError::Io)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
    use tokio::net::UnixListener;

    async fn create_test_server(socket_path: &str) -> UnixListener {
        let _ = std::fs::remove_file(socket_path);
        UnixListener::bind(socket_path).unwrap()
    }

    #[tokio::test]
    async fn test_client_config_defaults() {
        let config = ClientConfig {
            socket_path: "/tmp/test.sock".to_string(),
            daemon_name: "test-daemon".to_string(),
            ..Default::default()
        };

        assert_eq!(config.socket_path, "/tmp/test.sock");
        assert_eq!(config.daemon_name, "test-daemon");
        assert_eq!(config.timeout_seconds, 5);
        assert!(config.auto_reconnect);
        assert_eq!(config.buffer_size, 4096);
        assert!(config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_client_config_validation() {
        let invalid_config = ClientConfig {
            socket_path: "".to_string(),
            daemon_name: "test".to_string(),
            ..Default::default()
        };
        assert!(invalid_config.validate().is_err());

        let invalid_config2 = ClientConfig {
            socket_path: "/tmp/test.sock".to_string(),
            daemon_name: "".to_string(),
            ..Default::default()
        };
        assert!(invalid_config2.validate().is_err());
    }

    #[tokio::test]
    async fn test_log_client_connection() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("test.sock");
        let socket_str = socket_path.to_string_lossy().to_string();

        let listener = create_test_server(&socket_str).await;
        
        let _server_handle = tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        let mut buf = vec![0; 1024];
                        while let Ok(n) = stream.read(&mut buf).await {
                            if n == 0 { break; }
                        }
                    });
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = LogClient::connect(&socket_str, "test-daemon").await;
        assert!(client.is_ok());
        
        let client = client.unwrap();
        assert_eq!(client.config.daemon_name, "test-daemon");
        assert!(!client.hostname.is_empty());
    }

    #[tokio::test]
    async fn test_log_client_connection_timeout() {
        let socket_path = "/tmp/nonexistent_socket_12345.sock";
        let config = ClientConfig {
            socket_path: socket_path.to_string(),
            daemon_name: "test-daemon".to_string(),
            timeout_seconds: 1,
            ..Default::default()
        };

        let result = LogClient::with_config(config).await;
        match result {
            Err(LogStreamError::Connection(_)) => {},
            _ => panic!("Expected Connection error"),
        }
    }

    #[tokio::test]
    async fn test_all_log_levels() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("test_levels.sock");
        let socket_str = socket_path.to_string_lossy().to_string();

        let listener = create_test_server(&socket_str).await;
        let received_logs = Arc::new(Mutex::new(Vec::new()));
        let logs_clone = received_logs.clone();

        let _server_handle = tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let logs = logs_clone.clone();
                    tokio::spawn(async move {
                        let mut reader = BufReader::new(stream);
                        let mut line = String::new();
                        while let Ok(n) = reader.read_line(&mut line).await {
                            if n == 0 { break; }
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                logs.lock().await.push(trimmed.to_string());
                            }
                            line.clear();
                        }
                    });
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = LogClient::connect(&socket_str, "test-daemon").await.unwrap();

        // Test all log level methods
        client.emergency("Emergency message").await.unwrap();
        client.alert("Alert message").await.unwrap();
        client.critical("Critical message").await.unwrap();
        client.error("Error message").await.unwrap();
        client.warning("Warning message").await.unwrap();
        client.notice("Notice message").await.unwrap();
        client.info("Info message").await.unwrap();
        client.debug("Debug message").await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let logs = received_logs.lock().await;
        assert!(logs.len() >= 8);
    }

    #[tokio::test]
    async fn test_log_with_fields() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("test_fields.sock");
        let socket_str = socket_path.to_string_lossy().to_string();

        let listener = create_test_server(&socket_str).await;
        let received_logs = Arc::new(Mutex::new(Vec::new()));
        let logs_clone = received_logs.clone();

        let _server_handle = tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let logs = logs_clone.clone();
                    tokio::spawn(async move {
                        let mut buf = vec![0; 4096];
                        while let Ok(n) = stream.read(&mut buf).await {
                            if n == 0 { break; }
                            if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                                for line in s.lines() {
                                    if !line.is_empty() {
                                        logs.lock().await.push(line.to_string());
                                    }
                                }
                            }
                        }
                    });
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = LogClient::connect(&socket_str, "test-daemon").await.unwrap();

        let mut fields = HashMap::new();
        fields.insert("user_id".to_string(), "12345".to_string());
        fields.insert("request_id".to_string(), "req-67890".to_string());

        client.info_with_fields("User logged in", fields.clone()).await.unwrap();
        client.error_with_fields("Database error", fields.clone()).await.unwrap();
        client.warning_with_fields("High memory usage", fields).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let logs = received_logs.lock().await;
        assert!(logs.len() >= 3);
        
        for log in logs.iter() {
            assert!(log.contains("user_id"));
            assert!(log.contains("12345"));
            assert!(log.contains("request_id"));
            assert!(log.contains("req-67890"));
        }
    }

    #[tokio::test]
    async fn test_client_close() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("test_close.sock");
        let socket_str = socket_path.to_string_lossy().to_string();

        let listener = create_test_server(&socket_str).await;
        
        let _server_handle = tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        let mut buf = vec![0; 1024];
                        while let Ok(n) = stream.read(&mut buf).await {
                            if n == 0 { break; }
                        }
                    });
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = LogClient::connect(&socket_str, "test-daemon").await.unwrap();
        
        client.info("Test message before close").await.unwrap();
        assert!(client.close().await.is_ok());
        
        // After close, the connection should be None
        let conn_guard = client.connection.lock().await;
        assert!(conn_guard.is_none());
    }

    #[tokio::test]
    async fn test_log_entry_contains_metadata() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("test_metadata.sock");
        let socket_str = socket_path.to_string_lossy().to_string();

        let listener = create_test_server(&socket_str).await;
        let received_logs = Arc::new(Mutex::new(Vec::new()));
        let logs_clone = received_logs.clone();

        let _server_handle = tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let logs = logs_clone.clone();
                    tokio::spawn(async move {
                        let mut buf = vec![0; 4096];
                        while let Ok(n) = stream.read(&mut buf).await {
                            if n == 0 { break; }
                            if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                                for line in s.lines() {
                                    if !line.is_empty() {
                                        logs.lock().await.push(line.to_string());
                                    }
                                }
                            }
                        }
                    });
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = LogClient::connect(&socket_str, "metadata-test-daemon").await.unwrap();
        client.info("Test metadata").await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let logs = received_logs.lock().await;
        assert!(!logs.is_empty());
        
        let log_json = &logs[0];
        let parsed: serde_json::Value = serde_json::from_str(log_json).unwrap();
        
        assert_eq!(parsed["daemon"], "metadata-test-daemon");
        assert_eq!(parsed["level"], "Info");
        assert_eq!(parsed["message"], "Test metadata");
        assert!(parsed["pid"].is_number());
        assert!(parsed["hostname"].is_string());
        assert!(!parsed["hostname"].as_str().unwrap().is_empty());
        assert!(parsed["timestamp"].is_string());
        assert!(parsed["id"].is_string());
    }

    #[tokio::test]
    async fn test_reconnection_after_disconnect() {
        let temp_dir = tempdir().unwrap();
        let socket_path = temp_dir.path().join("test_reconnect.sock");
        let socket_str = socket_path.to_string_lossy().to_string();

        let listener = create_test_server(&socket_str).await;
        
        let _server_handle = tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        let mut buf = vec![0; 1024];
                        while let Ok(n) = stream.read(&mut buf).await {
                            if n == 0 { break; }
                        }
                    });
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = LogClient::connect(&socket_str, "test-daemon").await.unwrap();
        
        // Send first message
        client.info("First message").await.unwrap();
        
        // Force disconnect
        client.close().await.unwrap();
        
        // Try to send another message - should reconnect
        client.info("Message after reconnect").await.unwrap();
    }
}
