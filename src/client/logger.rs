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
