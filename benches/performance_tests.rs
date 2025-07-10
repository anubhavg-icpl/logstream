//! Performance benchmarks for LogStream

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use logstream::client::LogClient;
use logstream::config::ServerConfig;
use logstream::server::LogServer;
use logstream::types::{LogEntry, LogLevel};
use std::collections::HashMap;
use std::time::Duration;
use tempfile::tempdir;
use tokio::runtime::Runtime;

/// Benchmark single client throughput
fn bench_single_client_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("single_client_throughput");
    group.measurement_time(Duration::from_secs(5));
    
    group.bench_function("1000_messages", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = tempdir().unwrap();
            let socket_path = temp_dir.path().join("bench.sock");
            let mut config = ServerConfig::default();
            config.server.socket_path = socket_path.to_string_lossy().to_string();
            config.storage.output_directory = temp_dir.path().to_path_buf();

            let server = LogServer::new(config).await.unwrap();
            
            // Start server
            let server_handle = tokio::spawn(async move {
                server.start().await
            });

            tokio::time::sleep(Duration::from_millis(100)).await;

            let client = LogClient::connect(
                &socket_path.to_string_lossy(),
                "bench-client"
            ).await.unwrap();

            // Benchmark sending 1000 messages
            for i in 0..1000 {
                client.info(&format!("Benchmark message {}", i)).await.unwrap();
            }
            
            client.close().await.unwrap();
            server_handle.abort();
        });
    });
    
    group.finish();
}

/// Benchmark serialization/deserialization
fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");
    
    let entry = LogEntry::new(
        LogLevel::Info,
        "test-daemon".to_string(),
        "Test log message".to_string(),
    );

    group.bench_function("serialize", |b| {
        b.iter(|| entry.to_json())
    });

    let serialized = entry.to_json().unwrap();
    
    group.bench_function("deserialize", |b| {
        b.iter(|| LogEntry::from_json(&serialized))
    });
    
    group.finish();
}

criterion_group!(benches, bench_single_client_throughput, bench_serialization);
criterion_main!(benches);