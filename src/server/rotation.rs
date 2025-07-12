//! Log rotation implementation for LogStream

use crate::config::ServerConfig;
use crate::server::StorageBackend;
use crate::Result;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};

/// Log rotation manager
pub struct LogRotator {
    config: ServerConfig,
}

impl LogRotator {
    /// Create a new log rotator
    pub async fn new(config: &ServerConfig, _storage: Arc<StorageBackend>) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    /// Start the log rotation task
    pub async fn start_rotation_task(&self, mut shutdown_rx: broadcast::Receiver<()>) {
        if !self.config.storage.rotation.enabled {
            return;
        }

        let mut rotation_interval = interval(Duration::from_secs(3600));

        loop {
            tokio::select! {
                _ = rotation_interval.tick() => {
                    // Rotation logic would go here
                }
                _ = shutdown_rx.recv() => {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::time::timeout;

    async fn create_test_config(enabled: bool) -> ServerConfig {
        let temp_dir = tempdir().unwrap();
        let mut config = ServerConfig::default();
        config.storage.output_directory = temp_dir.path().to_path_buf();
        config.storage.rotation.enabled = enabled;
        config.storage.rotation.max_age_hours = 24 * 7; // 7 days
        config.storage.rotation.keep_files = 10;
        config
    }

    #[tokio::test]
    async fn test_log_rotator_creation() {
        let config = create_test_config(true).await;
        let storage = Arc::new(StorageBackend::new(&config).await.unwrap());
        
        let rotator = LogRotator::new(&config, storage).await;
        assert!(rotator.is_ok());
        
        let rotator = rotator.unwrap();
        assert!(rotator.config.storage.rotation.enabled);
        assert_eq!(rotator.config.storage.rotation.max_age_hours, 24 * 7);
        assert_eq!(rotator.config.storage.rotation.keep_files, 10);
    }

    #[tokio::test]
    async fn test_rotation_disabled() {
        let config = create_test_config(false).await;
        let storage = Arc::new(StorageBackend::new(&config).await.unwrap());
        let rotator = LogRotator::new(&config, storage).await.unwrap();
        
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        
        // Start rotation task
        let rotation_handle = tokio::spawn(async move {
            rotator.start_rotation_task(shutdown_rx).await;
        });
        
        // Give it a moment to run
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Send shutdown signal
        let _ = shutdown_tx.send(());
        
        // Should complete quickly since rotation is disabled
        let result = timeout(Duration::from_secs(1), rotation_handle).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rotation_enabled_with_shutdown() {
        let config = create_test_config(true).await;
        let storage = Arc::new(StorageBackend::new(&config).await.unwrap());
        let rotator = LogRotator::new(&config, storage).await.unwrap();
        
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        
        // Start rotation task
        let rotation_handle = tokio::spawn(async move {
            rotator.start_rotation_task(shutdown_rx).await;
        });
        
        // Let it run briefly
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Send shutdown signal
        let _ = shutdown_tx.send(());
        
        // Should respond to shutdown signal
        let result = timeout(Duration::from_secs(1), rotation_handle).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_rotators() {
        let config = create_test_config(true).await;
        let storage = Arc::new(StorageBackend::new(&config).await.unwrap());
        
        // Create multiple rotators with the same config
        let rotator1 = LogRotator::new(&config, storage.clone()).await.unwrap();
        let rotator2 = LogRotator::new(&config, storage.clone()).await.unwrap();
        
        // Verify they have the same config
        assert_eq!(rotator1.config.server.socket_path, rotator2.config.server.socket_path);
    }

    #[tokio::test]
    async fn test_rotation_config_values() {
        let mut config = create_test_config(true).await;
        config.storage.rotation.max_age_hours = 24 * 30; // 30 days
        config.storage.rotation.keep_files = 30;
        
        let storage = Arc::new(StorageBackend::new(&config).await.unwrap());
        let rotator = LogRotator::new(&config, storage).await.unwrap();
        
        assert_eq!(rotator.config.storage.rotation.max_age_hours, 24 * 30);
        assert_eq!(rotator.config.storage.rotation.keep_files, 30);
    }

    #[tokio::test]
    async fn test_rotation_task_lifecycle() {
        let config = create_test_config(true).await;
        let storage = Arc::new(StorageBackend::new(&config).await.unwrap());
        let rotator = LogRotator::new(&config, storage).await.unwrap();
        
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        
        // Clone shutdown_tx to keep channel alive
        let _shutdown_tx_clone = shutdown_tx.clone();
        
        // Start rotation task
        let rotation_handle = tokio::spawn(async move {
            rotator.start_rotation_task(shutdown_rx).await;
        });
        
        // Verify task is running
        assert!(!rotation_handle.is_finished());
        
        // Send shutdown
        let _ = shutdown_tx.send(());
        
        // Wait for completion
        let _ = rotation_handle.await;
    }

    #[tokio::test]
    async fn test_rotation_with_different_intervals() {
        // Test that we can create rotators with different configurations
        let configs = vec![
            (true, 24, 10),     // 1 day
            (true, 24 * 7, 20), // 7 days
            (true, 24 * 30, 30), // 30 days
            (false, 0, 0),
        ];
        
        for (enabled, hours, keep_files) in configs {
            let mut config = create_test_config(enabled).await;
            config.storage.rotation.max_age_hours = hours;
            config.storage.rotation.keep_files = keep_files;
            
            let storage = Arc::new(StorageBackend::new(&config).await.unwrap());
            let rotator = LogRotator::new(&config, storage).await.unwrap();
            
            assert_eq!(rotator.config.storage.rotation.enabled, enabled);
            assert_eq!(rotator.config.storage.rotation.max_age_hours, hours);
            assert_eq!(rotator.config.storage.rotation.keep_files, keep_files);
        }
    }
}
