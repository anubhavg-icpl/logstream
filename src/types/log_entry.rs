//! Log entry types and utilities

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

/// Type alias for log fields
pub type LogFields = HashMap<String, String>;

/// Log severity levels compatible with syslog and journald
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    /// Emergency: system is unusable
    Emergency = 0,
    /// Alert: action must be taken immediately
    Alert = 1,
    /// Critical: critical conditions
    Critical = 2,
    /// Error: error conditions
    Error = 3,
    /// Warning: warning conditions
    Warning = 4,
    /// Notice: normal but significant condition
    Notice = 5,
    /// Info: informational messages
    Info = 6,
    /// Debug: debug-level messages
    Debug = 7,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Emergency => write!(f, "EMERG"),
            LogLevel::Alert => write!(f, "ALERT"),
            LogLevel::Critical => write!(f, "CRIT"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Warning => write!(f, "WARN"),
            LogLevel::Notice => write!(f, "NOTICE"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Debug => write!(f, "DEBUG"),
        }
    }
}

/// A structured log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique identifier for this log entry
    pub id: Uuid,
    
    /// Timestamp when the log was created
    pub timestamp: DateTime<Utc>,
    
    /// Log severity level
    pub level: LogLevel,
    
    /// Name of the daemon/service that generated this log
    pub daemon: String,
    
    /// Primary log message
    pub message: String,
    
    /// Additional structured fields
    pub fields: LogFields,
    
    /// Process ID that generated the log
    pub pid: Option<u32>,
    
    /// Hostname where the log was generated
    pub hostname: Option<String>,
}

impl LogEntry {
    /// Create a new log entry with required fields
    pub fn new(level: LogLevel, daemon: String, message: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            level,
            daemon,
            message,
            fields: HashMap::new(),
            pid: None,
            hostname: None,
        }
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Format as human-readable string
    pub fn to_human_readable(&self) -> String {
        let timestamp = self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f");
        format!("{} {} {}: {}", timestamp, self.level, self.daemon, self.message)
    }
}
