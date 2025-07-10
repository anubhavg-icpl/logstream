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