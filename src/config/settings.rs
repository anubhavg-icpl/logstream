//! Configuration structures for LogStream

use crate::types::LogLevel;
use crate::{LogStreamError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server settings
    pub server: ServerSettings,
    /// Storage configuration
    pub storage: StorageSettings,
    /// Backend configurations
    pub backends: BackendSettings,
    /// Metrics configuration
    pub metrics: MetricsSettings,
}

/// Core server settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    /// Unix socket path to bind to
    pub socket_path: String,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Buffer size for reading data
    pub buffer_size: usize,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSettings {
    /// Directory to store log files
    pub output_directory: PathBuf,
    /// Maximum file size before rotation (bytes)
    pub max_file_size: u64,
    /// Log rotation settings
    pub rotation: RotationSettings,
}

/// Log rotation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationSettings {
    /// Enable log rotation
    pub enabled: bool,
    /// Maximum age of log files in hours
    pub max_age_hours: u32,
    /// Number of rotated files to keep
    pub keep_files: u32,
}

/// Backend configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendSettings {
    /// File storage backend
    pub file: FileBackendSettings,
    /// Journald backend
    pub journald: JournaldBackendSettings,
    /// Syslog backend  
    pub syslog: SyslogBackendSettings,
}

/// File backend settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBackendSettings {
    /// Enable file backend
    pub enabled: bool,
    /// File format (json, human, syslog)
    pub format: String,
    /// Enable compression
    pub compression: bool,
    /// Compression algorithm (gzip, lz4)
    pub compression_algorithm: String,
}

/// Journald backend settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JournaldBackendSettings {
    /// Enable journald backend
    pub enabled: bool,
    /// Syslog identifier for journald
    pub syslog_identifier: String,
}

/// Syslog backend settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyslogBackendSettings {
    /// Enable syslog backend
    pub enabled: bool,
    /// Syslog facility
    pub facility: String,
    /// Syslog server address (for remote syslog)
    pub server: Option<String>,
}

/// Metrics configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsSettings {
    /// Enable metrics endpoint
    pub enabled: bool,
    /// Metrics server port
    pub port: u16,
    /// Metrics endpoint path
    pub path: String,
}

/// Client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Server socket path to connect to
    pub socket_path: String,
    /// Client daemon name
    pub daemon_name: String,
    /// Minimum log level to send
    pub min_level: LogLevel,
    /// Connection timeout in seconds
    pub timeout_seconds: u64,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Buffer size for outgoing messages
    pub buffer_size: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: ServerSettings {
                socket_path: "/tmp/logstream.sock".to_string(),
                max_connections: 1000,
                buffer_size: 8192,
            },
            storage: StorageSettings {
                output_directory: PathBuf::from("/var/log/logstream"),
                max_file_size: 100 * 1024 * 1024, // 100MB
                rotation: RotationSettings {
                    enabled: true,
                    max_age_hours: 24,
                    keep_files: 7,
                },
            },
            backends: BackendSettings::default(),
            metrics: MetricsSettings::default(),
        }
    }
}

impl Default for FileBackendSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            format: "json".to_string(),
            compression: false,
            compression_algorithm: "gzip".to_string(),
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            socket_path: "/tmp/logstream.sock".to_string(),
            daemon_name: "unknown".to_string(),
            min_level: LogLevel::Info,
            timeout_seconds: 5,
            auto_reconnect: true,
            buffer_size: 4096,
        }
    }
}

impl ServerConfig {
    /// Load configuration from TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| LogStreamError::Config(format!("Failed to read config file: {}", e)))?;
        
        let config: ServerConfig = toml::from_str(&content)
            .map_err(|e| LogStreamError::Config(format!("Failed to parse config: {}", e)))?;
        
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.server.socket_path.is_empty() {
            return Err(LogStreamError::Config("Socket path cannot be empty".to_string()));
        }
        Ok(())
    }
}

impl ClientConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.socket_path.is_empty() {
            return Err(LogStreamError::Config("Socket path cannot be empty".to_string()));
        }
        if self.daemon_name.is_empty() {
            return Err(LogStreamError::Config("Daemon name cannot be empty".to_string()));
        }
        Ok(())
    }
}
