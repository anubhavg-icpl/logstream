//! Storage backend implementation for LogStream

use crate::config::ServerConfig;
use crate::types::LogEntry;
use crate::{LogStreamError, Result};
use dashmap::DashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::RwLock;

/// Storage backend for managing log files
pub struct StorageBackend {
    config: ServerConfig,
    file_writers: Arc<DashMap<String, Arc<RwLock<BufWriter<tokio::fs::File>>>>>,
}

impl StorageBackend {
    /// Create a new storage backend
    pub async fn new(config: &ServerConfig) -> Result<Self> {
        let file_writers = Arc::new(DashMap::new());
        Ok(Self {
            config: config.clone(),
            file_writers,
        })
    }

    /// Store a log entry
    pub async fn store_entry(&self, entry: LogEntry) -> Result<()> {
        if self.config.backends.file.enabled {
            self.store_to_file(&entry).await?;
        }
        Ok(())
    }

    async fn store_to_file(&self, entry: &LogEntry) -> Result<()> {
        let daemon_name = &entry.daemon;
        
        let writer = if let Some(existing) = self.file_writers.get(daemon_name) {
            Arc::clone(&*existing)
        } else {
            let file_path = self.get_log_file_path(daemon_name);
            let writer = self.create_file_writer(&file_path).await?;
            let writer_arc = Arc::new(RwLock::new(writer));
            self.file_writers.insert(daemon_name.clone(), Arc::clone(&writer_arc));
            writer_arc
        };

        let formatted_entry = match self.config.backends.file.format.as_str() {
            "json" => entry.to_json()?,
            _ => entry.to_human_readable(),
        };

        {
            let mut writer_guard = writer.write().await;
            writer_guard.write_all(formatted_entry.as_bytes()).await?;
            writer_guard.write_all(b"\n").await?;
            writer_guard.flush().await?;
        }

        Ok(())
    }

    fn get_log_file_path(&self, daemon_name: &str) -> PathBuf {
        self.config.storage.output_directory.join(format!("{}.log", daemon_name))
    }

    async fn create_file_writer(&self, file_path: &Path) -> Result<BufWriter<tokio::fs::File>> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)
            .await?;
        Ok(BufWriter::new(file))
    }
}
