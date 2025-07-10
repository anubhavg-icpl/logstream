//! LogStream server implementation

pub mod unix_socket;
pub mod rotation;
pub mod storage;

use crate::config::ServerConfig;
use crate::{LogStreamError, Result};
use std::sync::Arc;
use tokio::sync::broadcast;

pub use unix_socket::UnixSocketServer;
pub use rotation::LogRotator;
pub use storage::StorageBackend;

/// Main LogStream server that coordinates all components
pub struct LogServer {
    config: ServerConfig,
    storage: Arc<StorageBackend>,
    shutdown_tx: broadcast::Sender<()>,
}

impl LogServer {
    /// Create a new LogStream server with the given configuration
    pub async fn new(config: ServerConfig) -> Result<Self> {
        config.validate()?;

        let storage = Arc::new(StorageBackend::new(&config).await?);
        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config,
            storage,
            shutdown_tx,
        })
    }

    /// Start the LogStream server
    pub async fn start(&self) -> Result<()> {
        let unix_server = UnixSocketServer::new(
            &self.config,
            Arc::clone(&self.storage),
            self.shutdown_tx.subscribe(),
        ).await?;

        unix_server.start().await
    }
}
