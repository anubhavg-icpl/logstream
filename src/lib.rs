//! # LogStream - High-Performance Centralized Logging
//!
//! LogStream is a centralized logging solution designed for high-throughput, low-latency
//! log aggregation across multiple daemons and services.
//!
//! ## Features
//!
//! - **Multiple Backends**: Unix sockets, journald, syslog, file storage
//! - **High Performance**: Async I/O with Tokio, 13k+ requests/second
//! - **Log Rotation**: Size and time-based rotation with compression
//! - **Structured Logging**: JSON and custom format support
//! - **Thread-Safe**: Concurrent access from multiple processes
//!
//! ## Quick Start
//!
//! ### Server
//! ```no_run
//! use logstream::server::LogServer;
//! use logstream::config::ServerConfig;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ServerConfig::from_file("config/server.toml")?;
//!     let server = LogServer::new(config).await?;
//!     server.start().await?;
//!     Ok(())
//! }
//! ```
//!
//! ### Client
//! ```no_run
//! use logstream::client::LogClient;
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = LogClient::connect("/tmp/logstream.sock", "my-daemon").await?;
//!     
//!     let mut fields = HashMap::new();
//!     fields.insert("component".to_string(), "auth".to_string());
//!     
//!     client.info("Authentication successful", fields).await?;
//!     Ok(())
//! }
//! ```

#![deny(missing_docs)]
#![warn(clippy::all)]

pub mod client;
pub mod config;
pub mod server;
pub mod types;

/// Common error types used throughout LogStream
pub mod error {
    use std::fmt;

    /// LogStream error types
    #[derive(Debug)]
    pub enum LogStreamError {
        /// I/O operation failed
        Io(std::io::Error),
        /// Serialization/deserialization failed
        Serde(serde_json::Error),
        /// Configuration error
        Config(String),
        /// Server error
        Server(String),
        /// Client error
        Client(String),
        /// Connection error
        Connection(String),
    }

    impl fmt::Display for LogStreamError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                LogStreamError::Io(e) => write!(f, "I/O error: {}", e),
                LogStreamError::Serde(e) => write!(f, "Serialization error: {}", e),
                LogStreamError::Config(e) => write!(f, "Configuration error: {}", e),
                LogStreamError::Server(e) => write!(f, "Server error: {}", e),
                LogStreamError::Client(e) => write!(f, "Client error: {}", e),
                LogStreamError::Connection(e) => write!(f, "Connection error: {}", e),
            }
        }
    }

    impl std::error::Error for LogStreamError {}

    impl From<std::io::Error> for LogStreamError {
        fn from(err: std::io::Error) -> Self {
            LogStreamError::Io(err)
        }
    }

    impl From<serde_json::Error> for LogStreamError {
        fn from(err: serde_json::Error) -> Self {
            LogStreamError::Serde(err)
        }
    }

    /// Result type alias for LogStream operations
    pub type Result<T> = std::result::Result<T, LogStreamError>;
}

pub use error::{LogStreamError, Result};

/// Re-export commonly used types
pub mod prelude {
    pub use crate::client::{LogClient, LogLevel};
    pub use crate::config::{ClientConfig, ServerConfig};
    pub use crate::server::LogServer;
    pub use crate::types::{LogEntry, LogFields};
    pub use crate::{LogStreamError, Result};
}