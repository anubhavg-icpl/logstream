//! Performance benchmarks for LogStream

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use logstream::client::LogClient;
use logstream::config::ServerConfig;
use logstream::server::LogServer;
use logstream::types::{LogEntry, LogLevel};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

/// Benchmark single client throughput
fn bench_single_client_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("single_client_throughput");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10);
    
    for message_count in [100, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*message_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(message_count),
            message_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async move {
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

                    // Benchmark sending messages
                    for i in 0..count {
                        client.info(&format!("Benchmark message {}", i)).await.unwrap();
                    }
                    
                    client.close().await.unwrap();
                    server_handle.abort();
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent clients
fn bench_concurrent_clients(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_clients");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10);
    
    for client_count in [5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::new("clients", client_count),
            client_count,
            |b, &num_clients| {
                b.to_async(&rt).iter(|| async move {
                    let temp_dir = tempdir().unwrap();
                    let socket_path = temp_dir.path().join("concurrent.sock");
                    let mut config = ServerConfig::default();
                    config.server.socket_path = socket_path.to_string_lossy().to_string();
                    config.storage.output_directory = temp_dir.path().to_path_buf();

                    let server = LogServer::new(config).await.unwrap();
                    
                    // Start server
                    let server_handle = tokio::spawn(async move {
                        server.start().await
                    });

                    tokio::time::sleep(Duration::from_millis(200)).await;

                    // Launch concurrent clients
                    let mut handles = vec![];
                    for i in 0..num_clients {
                        let socket_str = socket_path.to_string_lossy().to_string();
                        let handle = tokio::spawn(async move {
                            let client = LogClient::connect(
                                &socket_str,
                                &format!("client-{}", i)
                            ).await.unwrap();
                            
                            for j in 0..100 {
                                client.info(&format!("Message {} from client {}", j, i)).await.unwrap();
                            }
                            
                            client.close().await.unwrap();
                        });
                        handles.push(handle);
                    }
                    
                    // Wait for all clients
                    for handle in handles {
                        handle.await.unwrap();
                    }
                    
                    server_handle.abort();
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark serialization/deserialization
fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");
    
    // Simple log entry
    let simple_entry = LogEntry::new(
        LogLevel::Info,
        "test-daemon".to_string(),
        "Test log message".to_string(),
    );

    // Complex log entry with fields
    let mut complex_entry = LogEntry::new(
        LogLevel::Error,
        "complex-daemon".to_string(),
        "Complex error message with detailed information".to_string(),
    );
    complex_entry.fields.insert("error_code".to_string(), "ERR_12345".to_string());
    complex_entry.fields.insert("user_id".to_string(), "user_67890".to_string());
    complex_entry.fields.insert("request_id".to_string(), "req_abcdef".to_string());
    complex_entry.fields.insert("stack_trace".to_string(), "at function1()\nat function2()\nat function3()".to_string());
    complex_entry.pid = Some(12345);
    complex_entry.hostname = Some("server-01.example.com".to_string());

    // Benchmark simple serialization
    group.bench_function("serialize_simple", |b| {
        b.iter(|| simple_entry.to_json())
    });

    // Benchmark complex serialization
    group.bench_function("serialize_complex", |b| {
        b.iter(|| complex_entry.to_json())
    });

    let simple_json = simple_entry.to_json().unwrap();
    let complex_json = complex_entry.to_json().unwrap();
    
    // Benchmark simple deserialization
    group.bench_function("deserialize_simple", |b| {
        b.iter(|| LogEntry::from_json(&simple_json))
    });

    // Benchmark complex deserialization
    group.bench_function("deserialize_complex", |b| {
        b.iter(|| LogEntry::from_json(&complex_json))
    });
    
    group.finish();
}

/// Benchmark message batching
fn bench_message_batching(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("message_batching");
    group.measurement_time(Duration::from_secs(10));
    
    for batch_size in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            batch_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async move {
                    let temp_dir = tempdir().unwrap();
                    let socket_path = temp_dir.path().join("batch.sock");
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
                        "batch-client"
                    ).await.unwrap();

                    // Send messages in batches
                    let total_messages = 1000;
                    for i in (0..total_messages).step_by(size) {
                        let batch_end = std::cmp::min(i + size, total_messages);
                        for j in i..batch_end {
                            client.info(&format!("Batch message {}", j)).await.unwrap();
                        }
                        // Small delay between batches
                        if batch_end < total_messages {
                            tokio::time::sleep(Duration::from_micros(10)).await;
                        }
                    }
                    
                    client.close().await.unwrap();
                    server_handle.abort();
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark structured logging overhead
fn bench_structured_logging(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("structured_logging");
    group.measurement_time(Duration::from_secs(5));
    
    group.bench_function("without_fields", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = tempdir().unwrap();
            let socket_path = temp_dir.path().join("no_fields.sock");
            let mut config = ServerConfig::default();
            config.server.socket_path = socket_path.to_string_lossy().to_string();
            config.storage.output_directory = temp_dir.path().to_path_buf();

            let server = LogServer::new(config).await.unwrap();
            let server_handle = tokio::spawn(async move {
                server.start().await
            });

            tokio::time::sleep(Duration::from_millis(100)).await;

            let client = LogClient::connect(
                &socket_path.to_string_lossy(),
                "bench-client"
            ).await.unwrap();

            for i in 0..100 {
                client.info(&format!("Simple message {}", i)).await.unwrap();
            }
            
            client.close().await.unwrap();
            server_handle.abort();
        });
    });
    
    group.bench_function("with_fields", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = tempdir().unwrap();
            let socket_path = temp_dir.path().join("with_fields.sock");
            let mut config = ServerConfig::default();
            config.server.socket_path = socket_path.to_string_lossy().to_string();
            config.storage.output_directory = temp_dir.path().to_path_buf();

            let server = LogServer::new(config).await.unwrap();
            let server_handle = tokio::spawn(async move {
                server.start().await
            });

            tokio::time::sleep(Duration::from_millis(100)).await;

            let client = LogClient::connect(
                &socket_path.to_string_lossy(),
                "bench-client"
            ).await.unwrap();

            for i in 0..100 {
                let mut fields = HashMap::new();
                fields.insert("iteration".to_string(), i.to_string());
                fields.insert("user_id".to_string(), "12345".to_string());
                fields.insert("session_id".to_string(), "sess_abcdef".to_string());
                
                client.info_with_fields(&format!("Structured message {}", i), fields).await.unwrap();
            }
            
            client.close().await.unwrap();
            server_handle.abort();
        });
    });
    
    group.finish();
}

/// Benchmark log level filtering performance
fn bench_log_levels(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("log_levels");
    
    let levels = vec![
        ("emergency", LogLevel::Emergency),
        ("error", LogLevel::Error),
        ("warning", LogLevel::Warning),
        ("info", LogLevel::Info),
        ("debug", LogLevel::Debug),
    ];
    
    for (name, level) in levels {
        group.bench_function(name, |b| {
            b.to_async(&rt).iter(|| async move {
                let temp_dir = tempdir().unwrap();
                let socket_path = temp_dir.path().join(format!("{}.sock", name));
                let mut config = ServerConfig::default();
                config.server.socket_path = socket_path.to_string_lossy().to_string();
                config.storage.output_directory = temp_dir.path().to_path_buf();

                let server = LogServer::new(config).await.unwrap();
                let server_handle = tokio::spawn(async move {
                    server.start().await
                });

                tokio::time::sleep(Duration::from_millis(100)).await;

                let client = LogClient::connect(
                    &socket_path.to_string_lossy(),
                    "level-bench"
                ).await.unwrap();

                for i in 0..100 {
                    client.log(level, &format!("Message {}", i), HashMap::new()).await.unwrap();
                }
                
                client.close().await.unwrap();
                server_handle.abort();
            });
        });
    }
    
    group.finish();
}

/// Benchmark memory usage with large messages
fn bench_large_messages(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("large_messages");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(10);
    
    for size_kb in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("size_kb", size_kb),
            size_kb,
            |b, &kb| {
                b.to_async(&rt).iter(|| async move {
                    let temp_dir = tempdir().unwrap();
                    let socket_path = temp_dir.path().join("large.sock");
                    let mut config = ServerConfig::default();
                    config.server.socket_path = socket_path.to_string_lossy().to_string();
                    config.storage.output_directory = temp_dir.path().to_path_buf();

                    let server = LogServer::new(config).await.unwrap();
                    let server_handle = tokio::spawn(async move {
                        server.start().await
                    });

                    tokio::time::sleep(Duration::from_millis(100)).await;

                    let client = LogClient::connect(
                        &socket_path.to_string_lossy(),
                        "large-msg-client"
                    ).await.unwrap();

                    // Create a large message
                    let large_content = "x".repeat(kb * 1024);
                    
                    for i in 0..10 {
                        client.info(&format!("Large message {}: {}", i, large_content)).await.unwrap();
                    }
                    
                    client.close().await.unwrap();
                    server_handle.abort();
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_single_client_throughput,
    bench_concurrent_clients,
    bench_serialization,
    bench_message_batching,
    bench_structured_logging,
    bench_log_levels,
    bench_large_messages
);
criterion_main!(benches);