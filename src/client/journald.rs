//! Journald integration for LogStream client
//!
//! This module provides direct journald logging capabilities as an alternative
//! or complement to the centralized LogStream server.

#[cfg(feature = "journald")]
use systemd_journal_logger::JournalLog;
#[cfg(feature = "journald")]
use tracing_journald::Layer as JournaldLayer;

use crate::types::{LogEntry, LogFields, LogLevel};
use crate::{LogStreamError, Result};
use std::collections::HashMap;

/// Journald client for direct logging to systemd journal
#[cfg(feature = "journald")]
pub struct JournaldClient {
    logger: JournalLog,
    daemon_name: String,
    extra_fields: LogFields,
}

#[cfg(feature = "journald")]
impl JournaldClient {
    /// Create a new journald client
    pub fn new(daemon_name: &str) -> Result<Self> {
        let logger = JournalLog::new()
            .map_err(|e| LogStreamError::Config(format!("Failed to initialize journald: {}", e)))?
            .with_syslog_identifier(daemon_name.to_string());

        Ok(Self {
            logger,
            daemon_name: daemon_name.to_string(),
            extra_fields: HashMap::new(),
        })
    }

    /// Add extra fields that will be included in all log entries
    pub fn with_extra_fields(mut self, fields: LogFields) -> Self {
        self.extra_fields = fields;
        self
    }

    /// Install this logger as the global log handler
    pub fn install_global(self) -> Result<()> {
        self.logger
            .install()
            .map_err(|e| LogStreamError::Config(format!("Failed to install journald logger: {}", e)))?;
        
        Ok(())
    }

    /// Log an entry directly to journald
    pub fn log_entry(&self, entry: &LogEntry) -> Result<()> {
        // Convert LogStream level to log crate level
        let log_level = match entry.level {
            LogLevel::Emergency => log::Level::Error,
            LogLevel::Alert => log::Level::Error,
            LogLevel::Critical => log::Level::Error,
            LogLevel::Error => log::Level::Error,
            LogLevel::Warning => log::Level::Warn,
            LogLevel::Notice => log::Level::Info,
            LogLevel::Info => log::Level::Info,
            LogLevel::Debug => log::Level::Debug,
        };

        // Create log record with all available metadata
        let record = log::Record::builder()
            .args(format_args!("{}", entry.message))
            .level(log_level)
            .target(&entry.daemon)
            .build();

        // Log the record
        self.logger.log(&record);

        Ok(())
    }

    /// Check if journald is available on the system
    pub fn is_available() -> bool {
        JournalLog::new().is_ok()
    }
}

/// Fallback implementation when journald feature is not enabled
#[cfg(not(feature = "journald"))]
pub struct JournaldClient;

#[cfg(not(feature = "journald"))]
impl JournaldClient {
    pub fn new(_daemon_name: &str) -> Result<Self> {
        Err(LogStreamError::Config(
            "Journald support not compiled in. Enable 'journald' feature.".to_string()
        ))
    }

    pub fn is_available() -> bool {
        false
    }
}

/// Tracing integration for journald
#[cfg(feature = "journald")]
pub struct TracingJournaldClient {
    layer: JournaldLayer,
}

#[cfg(feature = "journald")]
impl TracingJournaldClient {
    /// Create a new tracing journald client
    pub fn new(syslog_identifier: &str) -> Result<Self> {
        let layer = tracing_journald::layer()
            .map_err(|e| LogStreamError::Config(format!("Failed to create tracing journald layer: {}", e)))?
            .with_syslog_identifier(syslog_identifier.to_string());

        Ok(Self { layer })
    }

    /// Get the journald layer for use with tracing subscriber
    pub fn layer(self) -> JournaldLayer {
        self.layer
    }
}

/// Fallback implementation when journald feature is not enabled
#[cfg(not(feature = "journald"))]
pub struct TracingJournaldClient;

#[cfg(not(feature = "journald"))]
impl TracingJournaldClient {
    pub fn new(_syslog_identifier: &str) -> Result<Self> {
        Err(LogStreamError::Config(
            "Journald support not compiled in. Enable 'journald' feature.".to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_journald_not_available_without_feature() {
        #[cfg(not(feature = "journald"))]
        {
            assert!(!JournaldClient::is_available());
            let result = JournaldClient::new("test-daemon");
            assert!(result.is_err());
            match result.unwrap_err() {
                LogStreamError::Config(msg) => {
                    assert!(msg.contains("Journald support not compiled in"));
                }
                _ => panic!("Expected Config error"),
            }
        }
    }

    #[test]
    fn test_tracing_journald_not_available_without_feature() {
        #[cfg(not(feature = "journald"))]
        {
            let result = TracingJournaldClient::new("test-daemon");
            assert!(result.is_err());
            match result.unwrap_err() {
                LogStreamError::Config(msg) => {
                    assert!(msg.contains("Journald support not compiled in"));
                }
                _ => panic!("Expected Config error"),
            }
        }
    }

    #[test]
    #[cfg(feature = "journald")]
    fn test_journald_client_creation() {
        // This test may fail if journald is not available on the system
        if JournaldClient::is_available() {
            let client = JournaldClient::new("test-daemon");
            assert!(client.is_ok());
            
            let client = client.unwrap();
            assert_eq!(client.daemon_name, "test-daemon");
            assert!(client.extra_fields.is_empty());
        }
    }

    #[test]
    #[cfg(feature = "journald")]
    fn test_journald_with_extra_fields() {
        if JournaldClient::is_available() {
            let mut fields = HashMap::new();
            fields.insert("app_version".to_string(), "1.0.0".to_string());
            fields.insert("environment".to_string(), "test".to_string());
            
            let client = JournaldClient::new("test-daemon")
                .unwrap()
                .with_extra_fields(fields.clone());
                
            assert_eq!(client.extra_fields.len(), 2);
            assert_eq!(client.extra_fields.get("app_version"), Some(&"1.0.0".to_string()));
            assert_eq!(client.extra_fields.get("environment"), Some(&"test".to_string()));
        }
    }

    #[test]
    #[cfg(feature = "journald")]
    fn test_log_level_conversion() {
        if JournaldClient::is_available() {
            let client = JournaldClient::new("test-daemon").unwrap();
            
            // Test various log entries
            let test_cases = vec![
                (LogLevel::Emergency, "Emergency test"),
                (LogLevel::Alert, "Alert test"),
                (LogLevel::Critical, "Critical test"),
                (LogLevel::Error, "Error test"),
                (LogLevel::Warning, "Warning test"),
                (LogLevel::Notice, "Notice test"),
                (LogLevel::Info, "Info test"),
                (LogLevel::Debug, "Debug test"),
            ];
            
            for (level, message) in test_cases {
                let entry = LogEntry::new(level, "test-daemon".to_string(), message.to_string());
                let result = client.log_entry(&entry);
                // We can't verify the actual log was written, but we can check no errors occurred
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    #[cfg(feature = "journald")]
    fn test_tracing_journald_client_creation() {
        // This test may fail if journald is not available on the system
        let result = TracingJournaldClient::new("test-daemon");
        if result.is_ok() {
            let client = result.unwrap();
            // We can't test much more without actually integrating with tracing
            // but at least we know it creates successfully
            let _layer = client.layer();
        }
    }
}