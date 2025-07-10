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
