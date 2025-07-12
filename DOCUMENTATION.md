# LogStream - Comprehensive Documentation

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [API Reference](#api-reference)
4. [Configuration Reference](#configuration-reference)
5. [Communication Protocol](#communication-protocol)
6. [Deployment Guide](#deployment-guide)
7. [Operations Guide](#operations-guide)
8. [Examples](#examples)
9. [Troubleshooting](#troubleshooting)

## Overview

LogStream is a high-performance centralized logging solution designed for aggregating logs from multiple daemons and services. Built with Rust, it provides:

- **Multiple Backends**: Unix sockets, file storage, journald, syslog
- **High Performance**: Async I/O with Tokio, lock-free concurrent structures
- **Flexible Configuration**: TOML-based configuration with sensible defaults
- **Structured Logging**: JSON format with custom fields support
- **Automatic Rotation**: Size and age-based log rotation with compression
- **Monitoring**: Optional Prometheus metrics endpoint

### Key Features

- Zero-copy operations for maximum performance
- Automatic client reconnection with exponential backoff
- Multiple output formats (JSON, human-readable, syslog)
- Configurable compression (gzip/lz4) for rotated files
- Graceful shutdown handling
- Comprehensive error handling and validation

## Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            LogStream Architecture                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐               │
│  │   Client 1   │     │   Client 2   │     │   Client N   │               │
│  │  (Daemon A)  │     │  (Daemon B)  │     │  (Service X) │               │
│  └──────┬───────┘     └──────┬───────┘     └──────┬───────┘               │
│         │                     │                     │                       │
│         │   LogEntry JSON     │   LogEntry JSON     │   LogEntry JSON      │
│         └─────────────────────┴─────────────────────┘                      │
│                               │                                             │
│                               ▼                                             │
│         ┌───────────────────────────────────────────────┐                  │
│         │           Unix Socket Server                  │                  │
│         │        (/tmp/logstream.sock)                 │                  │
│         │  ┌─────────────────────────────────────┐    │                  │
│         │  │    Connection Handler (Async)        │    │                  │
│         │  │  - Accept connections                │    │                  │
│         │  │  - Spawn task per client             │    │                  │
│         │  │  - Parse JSON messages               │    │                  │
│         │  └─────────────────┬───────────────────┘    │                  │
│         └────────────────────┼─────────────────────────┘                  │
│                              │                                             │
│                              ▼                                             │
│         ┌───────────────────────────────────────────────┐                  │
│         │            LogServer Core                      │                  │
│         │  ┌─────────────────────────────────────┐     │                  │
│         │  │        Message Router                │     │                  │
│         │  │  - Validate entries                  │     │                  │
│         │  │  - Apply filters                     │     │                  │
│         │  │  - Route to backends                 │     │                  │
│         │  └─────────────────┬───────────────────┘     │                  │
│         └────────────────────┼─────────────────────────┘                  │
│                              │                                             │
│         ┌────────────────────┼────────────────────┐                       │
│         ▼                    ▼                    ▼                       │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐               │
│  │ File Backend │    │   Journald   │    │    Syslog    │               │
│  │              │    │   Backend    │    │   Backend    │               │
│  │ - JSON/Human │    │              │    │              │               │
│  │ - Rotation   │    │  (Optional)  │    │  (Optional)  │               │
│  │ - Compress   │    │              │    │              │               │
│  └──────┬───────┘    └──────────────┘    └──────────────┘               │
│         │                                                                  │
│         ▼                                                                  │
│  ┌──────────────┐                                                        │
│  │ Log Rotator  │────► Compressed Archives                               │
│  │              │      (*.gz or *.lz4)                                   │
│  └──────────────┘                                                        │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

### Threading Model

```
┌─────────────────────────────────────────────────┐
│              Main Thread                         │
│  - Server initialization                        │
│  - Shutdown signal handling                     │
└─────────────┬───────────────────────────────────┘
              │
              ├─── Spawns ───┐
              │              │
              ▼              ▼
┌──────────────────┐  ┌──────────────────┐
│ Socket Listener  │  │ Rotation Task    │
│    (Task 1)      │  │    (Task 2)      │
│                  │  │                  │
│ Per connection:  │  │ - Timer-based    │
│ spawn new task   │  │ - Checks files   │
└────────┬─────────┘  └──────────────────┘
         │
         ├─── Spawns per client ───┐
         │                         │
         ▼                         ▼
┌──────────────────┐      ┌──────────────────┐
│ Client Handler 1 │      │ Client Handler N │
│    (Task)        │      │    (Task)        │
│                  │      │                  │
│ - Read messages  │      │ - Read messages  │
│ - Process logs   │      │ - Process logs   │
│ - Send to backend│      │ - Send to backend│
└──────────────────┘      └──────────────────┘
```

### Data Flow

```
Client                    Server                    Storage
  │                         │                         │
  ├─────log("info", msg)───►│                         │
  │                         │                         │
  │                    Parse JSON                     │
  │                         │                         │
  │                    Create Entry                   │
  │                    - UUID                         │
  │                    - Timestamp                    │
  │                    - Hostname                     │
  │                         │                         │
  │                         ├────Store Entry─────────►│
  │                         │                         │
  │                         │                    Write to:
  │                         │                    - File
  │                         │                    - Journald
  │                         │                    - Syslog
  │                         │                         │
  │◄────────ACK─────────────┤◄────────Done───────────┤
  │                         │                         │
```

## API Reference

### Client API

#### LogClient

The main client for sending logs to the LogStream server.

```rust
pub struct LogClient { /* private fields */ }
```

##### Constructors

```rust
pub async fn connect(socket_path: &str, daemon_name: &str) -> Result<Self>
```
Create a new client with default configuration.

```rust
pub async fn with_config(config: ClientConfig) -> Result<Self>
```
Create a new client with custom configuration.

##### Logging Methods

```rust
// Core logging method
pub async fn log(&self, level: LogLevel, message: &str, fields: LogFields) -> Result<()>

// Convenience methods
pub async fn emergency<S: Into<String>>(&self, message: S) -> Result<()>
pub async fn alert<S: Into<String>>(&self, message: S) -> Result<()>
pub async fn critical<S: Into<String>>(&self, message: S) -> Result<()>
pub async fn error<S: Into<String>>(&self, message: S) -> Result<()>
pub async fn warning<S: Into<String>>(&self, message: S) -> Result<()>
pub async fn notice<S: Into<String>>(&self, message: S) -> Result<()>
pub async fn info<S: Into<String>>(&self, message: S) -> Result<()>
pub async fn debug<S: Into<String>>(&self, message: S) -> Result<()>

// Methods with structured fields
pub async fn critical_with_fields<S>(&self, message: S, fields: LogFields) -> Result<()>
pub async fn error_with_fields<S>(&self, message: S, fields: LogFields) -> Result<()>
pub async fn warning_with_fields<S>(&self, message: S, fields: LogFields) -> Result<()>
pub async fn info_with_fields<S>(&self, message: S, fields: LogFields) -> Result<()>
```

### Server API

#### LogServer

```rust
pub struct LogServer { /* private fields */ }

impl LogServer {
    pub async fn new(config: ServerConfig) -> Result<Self>
    pub async fn start(self) -> Result<()>
}
```

### Types

#### LogEntry

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub daemon: String,
    pub message: String,
    pub fields: LogFields,
    pub pid: Option<u32>,
    pub hostname: Option<String>,
}
```

#### LogLevel

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum LogLevel {
    Emergency = 0,  // System is unusable
    Alert = 1,      // Action must be taken immediately
    Critical = 2,   // Critical conditions
    Error = 3,      // Error conditions
    Warning = 4,    // Warning conditions
    Notice = 5,     // Normal but significant
    Info = 6,       // Informational messages
    Debug = 7,      // Debug-level messages
}
```

#### LogFields

```rust
pub type LogFields = HashMap<String, String>;
```

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum LogStreamError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Server error: {0}")]
    Server(String),
    
    #[error("Client error: {0}")]
    Client(String),
    
    #[error("Connection error: {0}")]
    Connection(String),
}

pub type Result<T> = std::result::Result<T, LogStreamError>;
```

## Configuration Reference

### Server Configuration

Server configuration uses TOML format with the following structure:

```toml
# /etc/logstream/server.toml

[server]
socket_path = "/tmp/logstream.sock"        # Unix socket path
max_connections = 1000                     # Maximum concurrent connections
buffer_size = 8192                         # Buffer size in bytes

[storage]
output_directory = "/var/log/logstream"    # Log storage directory
max_file_size = 104857600                  # Max file size before rotation (100MB)

[storage.rotation]
enabled = true                             # Enable log rotation
max_age_hours = 24                         # Maximum age of log files
keep_files = 7                             # Number of rotated files to keep

[backends.file]
enabled = true                             # Enable file backend
format = "json"                            # Output format: json, human, syslog
compression = false                        # Enable compression for rotated files
compression_algorithm = "gzip"             # Algorithm: gzip or lz4

[backends.journald]
enabled = false                            # Enable journald backend
syslog_identifier = "logstream"            # Syslog identifier

[backends.syslog]
enabled = false                            # Enable syslog backend
facility = "LOG_USER"                      # Syslog facility
server = null                              # Remote syslog server (optional)

[metrics]
enabled = false                            # Enable metrics endpoint
port = 9090                                # Metrics server port
path = "/metrics"                          # Metrics endpoint path
```

### Client Configuration

```rust
pub struct ClientConfig {
    pub socket_path: String,        // Server socket path
    pub daemon_name: String,        // Client identifier
    pub min_level: LogLevel,        // Minimum log level
    pub timeout_seconds: u64,       // Connection timeout
    pub auto_reconnect: bool,       // Enable auto-reconnection
    pub buffer_size: usize,         // Message buffer size
}
```

### Command-Line Options

```bash
logstream-server [OPTIONS]

OPTIONS:
    -c, --config <CONFIG>           Configuration file path [default: config/server.toml]
    -s, --socket <SOCKET>           Socket path to bind to (overrides config)
    -o, --output <OUTPUT>           Log output directory (overrides config)
    -v, --verbose                   Enable verbose logging
        --journald                  Enable journald backend
        --metrics                   Enable metrics endpoint
        --metrics-port <PORT>       Metrics port [default: 9090]
```

## Communication Protocol

### Overview

LogStream uses a newline-delimited JSON protocol over Unix domain sockets.

### Message Format

Each log message is a JSON object on a single line:

```json
{"id":"550e8400-e29b-41d4-a716-446655440000","timestamp":"2024-01-15T10:30:45.123Z","level":6,"daemon":"web-server","message":"Request processed","fields":{"user_id":"12345"},"pid":1234,"hostname":"server01"}\n
```

### Connection Flow

```
Client                          Server
  │                               │
  ├──── Connect to socket ────────►
  │     /tmp/logstream.sock       │
  │                               │
  ◄──── Accept connection ─────────┤
  │                               │
  ├──── Send daemon name ─────────►
  │     "daemon-name\n"           │
  │                               │
  ◄──── Connection ready ──────────┤
  │                               │
  ├──── Send log messages ────────►
  │     {"id":"...","level":6...}\n
  │                               │
```

### Reconnection

Clients implement automatic reconnection with exponential backoff:
- Initial delay: 100ms
- Max delay: 5 seconds
- Backoff factor: 2x

## Deployment Guide

### System Requirements

#### Minimum
- OS: Linux (kernel 3.10+), macOS 10.14+
- CPU: 1 core
- RAM: 256MB
- Disk: 1GB free space

#### Recommended
- OS: Linux kernel 5.0+ with systemd
- CPU: 2+ cores
- RAM: 1GB+
- Disk: 10GB+ for log storage

### Installation

#### Building from Source

```bash
# Clone and build
git clone https://github.com/yourusername/logstream
cd logstream
cargo build --release --all-features

# Install
sudo cp target/release/logstream-server /usr/local/bin/
sudo chmod +x /usr/local/bin/logstream-server
```

#### System Setup

```bash
# Create user and directories
sudo groupadd -r logstream
sudo useradd -r -g logstream -s /sbin/nologin logstream
sudo mkdir -p /etc/logstream /var/log/logstream /var/run/logstream

# Set permissions
sudo chown -R logstream:logstream /var/log/logstream
sudo chown -R logstream:logstream /var/run/logstream
```

### Systemd Service

Create `/etc/systemd/system/logstream.service`:

```ini
[Unit]
Description=LogStream Centralized Logging Server
After=network.target

[Service]
Type=simple
User=logstream
Group=logstream
ExecStart=/usr/local/bin/logstream-server -c /etc/logstream/server.toml
Restart=always
RestartSec=5

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/logstream /var/run/logstream

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable logstream
sudo systemctl start logstream
```

## Operations Guide

### Monitoring

#### Health Checks

```bash
# Service status
systemctl status logstream

# Socket connectivity
nc -zU /tmp/logstream.sock && echo "Socket is listening"

# Log activity
tail -f /var/log/logstream/current.log
```

#### Metrics

If metrics are enabled, monitor at `http://localhost:9090/metrics`:
- `logstream_connections_total`
- `logstream_messages_received_total`
- `logstream_messages_processed_total`
- `logstream_errors_total`

### Log Rotation

Configure automatic rotation with logrotate:

```
# /etc/logrotate.d/logstream
/var/log/logstream/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 0640 logstream logstream
}
```

### Performance Tuning

#### System Limits

```bash
# Increase file descriptors
echo "logstream soft nofile 65536" >> /etc/security/limits.conf
echo "logstream hard nofile 65536" >> /etc/security/limits.conf

# Kernel parameters
cat >> /etc/sysctl.d/99-logstream.conf << 'EOF'
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.unix.max_dgram_qlen = 1000
fs.file-max = 1000000
EOF
```

#### Configuration Optimization

```toml
[server]
max_connections = 10000
buffer_size = 16384

[storage]
max_file_size = 2147483648  # 2GB

[backends.file]
compression = true
compression_algorithm = "lz4"  # Faster than gzip
```

## Examples

### Basic Client Usage

```rust
use logstream::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to server
    let client = LogClient::connect("/tmp/logstream.sock", "my-app").await?;
    
    // Simple logging
    client.info("Application started").await?;
    client.warning("Low memory warning").await?;
    
    // Structured logging
    let mut fields = HashMap::new();
    fields.insert("user_id".to_string(), "12345".to_string());
    fields.insert("action".to_string(), "login".to_string());
    
    client.info_with_fields("User authenticated", fields).await?;
    
    Ok(())
}
```

### Server Setup

```rust
use logstream::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = ServerConfig::from_file("server.toml")?;
    
    // Create and start server
    let server = LogServer::new(config).await?;
    server.start().await?;
    
    Ok(())
}
```

### Multi-Daemon Example

```rust
use logstream::prelude::*;
use tokio::task;

#[tokio::main]
async fn main() -> Result<()> {
    let mut handles = vec![];
    
    // Spawn multiple daemon simulators
    for i in 0..5 {
        let handle = task::spawn(async move {
            let daemon_name = format!("daemon-{}", i);
            let client = LogClient::connect("/tmp/logstream.sock", &daemon_name).await?;
            
            // Simulate daemon activity
            for j in 0..100 {
                match j % 5 {
                    0 => client.info("Regular operation").await?,
                    1 => client.debug("Debug information").await?,
                    2 => client.warning("Performance warning").await?,
                    3 => client.error("Recoverable error").await?,
                    _ => client.notice("Status update").await?,
                }
                
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            
            Ok::<(), LogStreamError>(())
        });
        
        handles.push(handle);
    }
    
    // Wait for all daemons
    for handle in handles {
        handle.await??;
    }
    
    Ok(())
}
```

## Troubleshooting

### Common Issues

#### Service Won't Start

```bash
# Check logs
journalctl -u logstream -n 50

# Verify permissions
ls -la /tmp/logstream.sock
ls -la /var/log/logstream/

# Validate configuration
logstream-server -c /etc/logstream/server.toml --validate
```

#### Clients Can't Connect

```bash
# Check socket exists
test -S /tmp/logstream.sock && echo "Socket exists"

# Test connection
echo '{"test":"message"}' | nc -U /tmp/logstream.sock

# Check permissions
stat /tmp/logstream.sock
```

#### Performance Issues

```bash
# Enable debug logging
RUST_LOG=debug logstream-server -c /etc/logstream/server.toml

# Monitor connections
ss -x | grep logstream.sock | wc -l

# Run benchmarks
./scripts/benchmark.sh
```

### Debug Mode

Enable detailed logging:
```bash
# All modules
RUST_LOG=debug logstream-server

# Specific modules
RUST_LOG=logstream::server=debug,logstream::client=trace logstream-server
```

---

This documentation provides a comprehensive guide to understanding, deploying, and operating LogStream. For the latest updates and additional resources, visit the [project repository](https://github.com/yourusername/logstream).