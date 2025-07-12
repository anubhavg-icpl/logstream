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
    
    /// Deserialize from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Emergency < LogLevel::Alert);
        assert!(LogLevel::Alert < LogLevel::Critical);
        assert!(LogLevel::Critical < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Warning);
        assert!(LogLevel::Warning < LogLevel::Notice);
        assert!(LogLevel::Notice < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Debug);
    }

    #[test]
    fn test_log_level_values() {
        assert_eq!(LogLevel::Emergency as u8, 0);
        assert_eq!(LogLevel::Alert as u8, 1);
        assert_eq!(LogLevel::Critical as u8, 2);
        assert_eq!(LogLevel::Error as u8, 3);
        assert_eq!(LogLevel::Warning as u8, 4);
        assert_eq!(LogLevel::Notice as u8, 5);
        assert_eq!(LogLevel::Info as u8, 6);
        assert_eq!(LogLevel::Debug as u8, 7);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Emergency.to_string(), "EMERG");
        assert_eq!(LogLevel::Alert.to_string(), "ALERT");
        assert_eq!(LogLevel::Critical.to_string(), "CRIT");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
        assert_eq!(LogLevel::Warning.to_string(), "WARN");
        assert_eq!(LogLevel::Notice.to_string(), "NOTICE");
        assert_eq!(LogLevel::Info.to_string(), "INFO");
        assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
    }

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new(
            LogLevel::Info,
            "test-daemon".to_string(),
            "Test message".to_string(),
        );

        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.daemon, "test-daemon");
        assert_eq!(entry.message, "Test message");
        assert!(entry.fields.is_empty());
        assert!(entry.pid.is_none());
        assert!(entry.hostname.is_none());
        assert!(entry.timestamp <= Utc::now());
    }

    #[test]
    fn test_log_entry_with_fields() {
        let mut entry = LogEntry::new(
            LogLevel::Error,
            "app-daemon".to_string(),
            "Database connection failed".to_string(),
        );

        entry.fields.insert("error_code".to_string(), "DB001".to_string());
        entry.fields.insert("retry_count".to_string(), "3".to_string());
        entry.pid = Some(12345);
        entry.hostname = Some("server01".to_string());

        assert_eq!(entry.fields.len(), 2);
        assert_eq!(entry.fields.get("error_code"), Some(&"DB001".to_string()));
        assert_eq!(entry.fields.get("retry_count"), Some(&"3".to_string()));
        assert_eq!(entry.pid, Some(12345));
        assert_eq!(entry.hostname, Some("server01".to_string()));
    }

    #[test]
    fn test_log_entry_serialization() {
        let mut entry = LogEntry::new(
            LogLevel::Warning,
            "test-service".to_string(),
            "Memory usage high".to_string(),
        );
        entry.fields.insert("memory_percent".to_string(), "85".to_string());

        let json = entry.to_json().unwrap();
        assert!(json.contains("\"level\":\"Warning\""));
        assert!(json.contains("\"daemon\":\"test-service\""));
        assert!(json.contains("\"message\":\"Memory usage high\""));
        assert!(json.contains("\"memory_percent\":\"85\""));
    }

    #[test]
    fn test_log_entry_deserialization() {
        let mut original = LogEntry::new(
            LogLevel::Critical,
            "auth-daemon".to_string(),
            "Authentication failed".to_string(),
        );
        original.fields.insert("user".to_string(), "admin".to_string());
        original.pid = Some(5678);

        let json = original.to_json().unwrap();
        let deserialized = LogEntry::from_json(&json).unwrap();

        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.level, LogLevel::Critical);
        assert_eq!(deserialized.daemon, "auth-daemon");
        assert_eq!(deserialized.message, "Authentication failed");
        assert_eq!(deserialized.fields.get("user"), Some(&"admin".to_string()));
        assert_eq!(deserialized.pid, Some(5678));
    }

    #[test]
    fn test_log_entry_human_readable() {
        let entry = LogEntry::new(
            LogLevel::Info,
            "web-server".to_string(),
            "Request processed successfully".to_string(),
        );

        let readable = entry.to_human_readable();
        assert!(readable.contains("INFO"));
        assert!(readable.contains("web-server"));
        assert!(readable.contains("Request processed successfully"));
        // Check for timestamp format (YYYY-MM-DD HH:MM:SS.SSS)
        assert!(readable.chars().filter(|&c| c == '-').count() >= 2);
        assert!(readable.chars().filter(|&c| c == ':').count() >= 3);
    }

    #[test]
    fn test_multiple_log_entries_unique_ids() {
        let entry1 = LogEntry::new(LogLevel::Info, "daemon1".to_string(), "msg1".to_string());
        let entry2 = LogEntry::new(LogLevel::Info, "daemon1".to_string(), "msg1".to_string());
        
        assert_ne!(entry1.id, entry2.id);
    }

    #[test]
    fn test_log_fields_type_alias() {
        let mut fields: LogFields = HashMap::new();
        fields.insert("key1".to_string(), "value1".to_string());
        fields.insert("key2".to_string(), "value2".to_string());
        
        assert_eq!(fields.len(), 2);
        assert_eq!(fields.get("key1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_serialization_round_trip() {
        let mut original = LogEntry::new(
            LogLevel::Debug,
            "test-daemon".to_string(),
            "Debug message with special chars: \"quotes\", 'apostrophes', \\backslash".to_string(),
        );
        original.fields.insert("field_with_newline".to_string(), "line1\nline2".to_string());
        original.fields.insert("field_with_tab".to_string(), "col1\tcol2".to_string());
        original.pid = Some(99999);
        original.hostname = Some("test-host.example.com".to_string());

        let json = original.to_json().unwrap();
        let deserialized = LogEntry::from_json(&json).unwrap();

        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.timestamp, original.timestamp);
        assert_eq!(deserialized.level, original.level);
        assert_eq!(deserialized.daemon, original.daemon);
        assert_eq!(deserialized.message, original.message);
        assert_eq!(deserialized.fields, original.fields);
        assert_eq!(deserialized.pid, original.pid);
        assert_eq!(deserialized.hostname, original.hostname);
    }
}
