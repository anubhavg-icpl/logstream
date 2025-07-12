//! Storage backend implementation for LogStream

use crate::config::ServerConfig;
use crate::types::LogEntry;
use crate::Result;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LogLevel;
    use tempfile::tempdir;
    use tokio::fs;

    async fn create_test_config(dir: &Path) -> ServerConfig {
        let mut config = ServerConfig::default();
        config.storage.output_directory = dir.to_path_buf();
        config.backends.file.enabled = true;
        config.backends.file.format = "json".to_string();
        config
    }

    #[tokio::test]
    async fn test_storage_backend_creation() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config(temp_dir.path()).await;
        
        let backend = StorageBackend::new(&config).await;
        assert!(backend.is_ok());
        
        let backend = backend.unwrap();
        assert_eq!(backend.config.storage.output_directory, temp_dir.path());
        assert!(backend.file_writers.is_empty());
    }

    #[tokio::test]
    async fn test_store_entry_creates_file() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config(temp_dir.path()).await;
        let backend = StorageBackend::new(&config).await.unwrap();
        
        let entry = LogEntry::new(
            LogLevel::Info,
            "test-daemon".to_string(),
            "Test message".to_string(),
        );
        
        let result = backend.store_entry(entry).await;
        assert!(result.is_ok());
        
        // Check that file was created
        let log_file = temp_dir.path().join("test-daemon.log");
        assert!(log_file.exists());
        
        // Read and verify content
        let content = fs::read_to_string(log_file).await.unwrap();
        assert!(content.contains("Test message"));
        assert!(content.contains("test-daemon"));
        assert!(content.contains("Info"));
    }

    #[tokio::test]
    async fn test_store_multiple_entries() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config(temp_dir.path()).await;
        let backend = StorageBackend::new(&config).await.unwrap();
        
        // Store multiple entries
        for i in 0..5 {
            let entry = LogEntry::new(
                LogLevel::Info,
                "multi-daemon".to_string(),
                format!("Message {}", i),
            );
            backend.store_entry(entry).await.unwrap();
        }
        
        // Verify all entries were written
        let log_file = temp_dir.path().join("multi-daemon.log");
        let content = fs::read_to_string(log_file).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 5);
        
        for i in 0..5 {
            assert!(content.contains(&format!("Message {}", i)));
        }
    }

    #[tokio::test]
    async fn test_multiple_daemons() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config(temp_dir.path()).await;
        let backend = StorageBackend::new(&config).await.unwrap();
        
        // Store entries from different daemons
        let daemons = vec!["daemon1", "daemon2", "daemon3"];
        for daemon in &daemons {
            let entry = LogEntry::new(
                LogLevel::Info,
                daemon.to_string(),
                format!("Message from {}", daemon),
            );
            backend.store_entry(entry).await.unwrap();
        }
        
        // Verify separate files were created
        for daemon in &daemons {
            let log_file = temp_dir.path().join(format!("{}.log", daemon));
            assert!(log_file.exists());
            
            let content = fs::read_to_string(log_file).await.unwrap();
            assert!(content.contains(&format!("Message from {}", daemon)));
        }
        
        // Verify we have 3 writers cached
        assert_eq!(backend.file_writers.len(), 3);
    }

    #[tokio::test]
    async fn test_json_format() {
        let temp_dir = tempdir().unwrap();
        let mut config = create_test_config(temp_dir.path()).await;
        config.backends.file.format = "json".to_string();
        
        let backend = StorageBackend::new(&config).await.unwrap();
        
        let mut entry = LogEntry::new(
            LogLevel::Error,
            "json-test".to_string(),
            "JSON formatted message".to_string(),
        );
        entry.fields.insert("error_code".to_string(), "E001".to_string());
        
        backend.store_entry(entry).await.unwrap();
        
        let log_file = temp_dir.path().join("json-test.log");
        let content = fs::read_to_string(log_file).await.unwrap();
        
        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["level"], "Error");
        assert_eq!(parsed["daemon"], "json-test");
        assert_eq!(parsed["message"], "JSON formatted message");
        assert_eq!(parsed["fields"]["error_code"], "E001");
    }

    #[tokio::test]
    async fn test_human_readable_format() {
        let temp_dir = tempdir().unwrap();
        let mut config = create_test_config(temp_dir.path()).await;
        config.backends.file.format = "human".to_string();
        
        let backend = StorageBackend::new(&config).await.unwrap();
        
        let entry = LogEntry::new(
            LogLevel::Warning,
            "human-test".to_string(),
            "Human readable message".to_string(),
        );
        
        backend.store_entry(entry).await.unwrap();
        
        let log_file = temp_dir.path().join("human-test.log");
        let content = fs::read_to_string(log_file).await.unwrap();
        
        // Verify human readable format
        assert!(content.contains("WARN"));
        assert!(content.contains("human-test"));
        assert!(content.contains("Human readable message"));
        // Should not be JSON
        assert!(serde_json::from_str::<serde_json::Value>(content.trim()).is_err());
    }

    #[tokio::test]
    async fn test_disabled_file_backend() {
        let temp_dir = tempdir().unwrap();
        let mut config = create_test_config(temp_dir.path()).await;
        config.backends.file.enabled = false;
        
        let backend = StorageBackend::new(&config).await.unwrap();
        
        let entry = LogEntry::new(
            LogLevel::Info,
            "disabled-test".to_string(),
            "Should not be written".to_string(),
        );
        
        backend.store_entry(entry).await.unwrap();
        
        // No file should be created when backend is disabled
        let log_file = temp_dir.path().join("disabled-test.log");
        assert!(!log_file.exists());
    }

    #[tokio::test]
    async fn test_concurrent_writes() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config(temp_dir.path()).await;
        let backend = Arc::new(StorageBackend::new(&config).await.unwrap());
        
        let mut handles = vec![];
        
        // Spawn multiple tasks writing to the same daemon
        for i in 0..10 {
            let backend_clone = backend.clone();
            let handle = tokio::spawn(async move {
                let entry = LogEntry::new(
                    LogLevel::Info,
                    "concurrent-test".to_string(),
                    format!("Concurrent message {}", i),
                );
                backend_clone.store_entry(entry).await
            });
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        
        // Verify all messages were written
        let log_file = temp_dir.path().join("concurrent-test.log");
        let content = fs::read_to_string(log_file).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 10);
        
        // All messages should be present (order may vary)
        for i in 0..10 {
            assert!(content.contains(&format!("Concurrent message {}", i)));
        }
    }

    #[tokio::test]
    async fn test_get_log_file_path() {
        let temp_dir = tempdir().unwrap();
        let config = create_test_config(temp_dir.path()).await;
        let backend = StorageBackend::new(&config).await.unwrap();
        
        let path = backend.get_log_file_path("test-daemon");
        assert_eq!(path, temp_dir.path().join("test-daemon.log"));
        
        let path2 = backend.get_log_file_path("another-daemon");
        assert_eq!(path2, temp_dir.path().join("another-daemon.log"));
    }
}
