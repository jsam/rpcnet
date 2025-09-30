use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use rpcnet::migration::{
    models::ConnectionStateSnapshot,
    state::{
        encryption::{EncryptionKey, EncryptionService},
        serialization::{SerializationConfig, CompressionMethod, SerializationService},
    },
    auth::token_manager::{MigrationTokenManager, TokenValidationConfig},
};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;

/// Benchmark encryption and decryption performance
fn encryption_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("migration_encryption");
    let encryption_service = EncryptionService::new();
    let key = EncryptionKey::generate();
    
    // Test different data sizes
    let sizes = vec![1024, 10_240, 102_400, 1_024_000]; // 1KB to 1MB
    
    for size in sizes {
        let data = vec![0u8; size];
        
        group.bench_with_input(
            BenchmarkId::new("encrypt", size),
            &size,
            |b, _| {
                b.iter(|| {
                    encryption_service.encrypt(black_box(&data), black_box(&key))
                        .expect("Encryption failed")
                })
            },
        );
        
        let encrypted = encryption_service.encrypt(&data, &key).expect("Encryption failed");
        group.bench_with_input(
            BenchmarkId::new("decrypt", size),
            &size,
            |b, _| {
                b.iter(|| {
                    encryption_service.decrypt(black_box(&encrypted), black_box(&key))
                        .expect("Decryption failed")
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark serialization performance with different compression methods
fn serialization_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("migration_serialization");
    
    // Create test snapshots of different sizes
    let sizes = vec![100, 1_000, 10_000]; // Number of stream buffers
    let compression_methods = vec![
        CompressionMethod::None,
        CompressionMethod::Gzip,
        CompressionMethod::Zstd,
    ];
    
    for num_streams in sizes {
        for compression in &compression_methods {
            let config = SerializationConfig {
                compression: compression.clone(),
                chunk_size: 64 * 1024, // 64KB chunks
            };
            let service = SerializationService::new(config);
            
            // Create snapshot with multiple stream buffers
            let mut snapshot = ConnectionStateSnapshot::new(
                format!("conn-bench-{}", num_streams),
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
            );
            
            for i in 0..num_streams {
                let buffer = vec![i as u8; 1024]; // 1KB per buffer
                snapshot = snapshot.add_stream_buffer(i as u64, buffer);
            }
            
            group.bench_with_input(
                BenchmarkId::new(format!("serialize_{:?}", compression), num_streams),
                &num_streams,
                |b, _| {
                    b.to_async(&rt).iter(|| async {
                        service.serialize(black_box(&snapshot)).await
                            .expect("Serialization failed")
                    })
                },
            );
            
            // Pre-serialize for deserialization benchmark
            let serialized = rt.block_on(async {
                service.serialize(&snapshot).await.expect("Serialization failed")
            });
            
            group.bench_with_input(
                BenchmarkId::new(format!("deserialize_{:?}", compression), num_streams),
                &num_streams,
                |b, _| {
                    b.to_async(&rt).iter(|| async {
                        let _: ConnectionStateSnapshot = service.deserialize(black_box(&serialized)).await
                            .expect("Deserialization failed");
                    })
                },
            );
        }
    }
    
    group.finish();
}

/// Benchmark token generation and validation
fn token_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("migration_tokens");
    
    let config = TokenValidationConfig {
        max_requests_per_second: 1000,
        token_expiry_seconds: 3600,
        secret_key: b"benchmark_secret_key_32_bytes_long".to_vec(),
    };
    let token_manager = MigrationTokenManager::new(config);
    
    // Create a sample migration request
    let migration_request = rpcnet::migration::models::MigrationRequest::new(
        "benchmark-migration".to_string(),
        "benchmark-connection".to_string(),
        "source-worker".to_string(),
        "target-worker".to_string(),
        vec![1, 2, 3, 4, 5],
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
    );
    
    group.bench_function("token_generation", |b| {
        b.iter(|| {
            token_manager.generate_token(black_box(&migration_request))
                .expect("Token generation failed")
        })
    });
    
    let token = token_manager.generate_token(&migration_request)
        .expect("Token generation failed");
    
    group.bench_function("token_validation", |b| {
        b.iter(|| {
            token_manager.validate_token(black_box(&token), black_box(&migration_request))
                .expect("Token validation failed")
        })
    });
    
    group.finish();
}

/// Benchmark complete migration workflow
fn complete_workflow_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("migration_complete_workflow");
    group.sample_size(10); // Fewer samples for complex benchmark
    
    let encryption_service = EncryptionService::new();
    let key = EncryptionKey::generate();
    let serialization_service = SerializationService::new(SerializationConfig {
        compression: CompressionMethod::Gzip,
        chunk_size: 64 * 1024,
    });
    let token_manager = MigrationTokenManager::new(TokenValidationConfig {
        max_requests_per_second: 1000,
        token_expiry_seconds: 3600,
        secret_key: b"workflow_benchmark_secret_key_32b".to_vec(),
    });
    
    // Test different state sizes
    let state_sizes = vec![10, 100, 500]; // Number of stream buffers
    
    for num_streams in state_sizes {
        group.bench_with_input(
            BenchmarkId::new("complete_workflow", num_streams),
            &num_streams,
            |b, &num_streams| {
                b.to_async(&rt).iter(|| async {
                    // Create connection state snapshot
                    let mut snapshot = ConnectionStateSnapshot::new(
                        format!("workflow-conn-{}", num_streams),
                        SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
                    );
                    
                    for i in 0..num_streams {
                        let buffer = vec![i as u8; 1024];
                        snapshot = snapshot.add_stream_buffer(i as u64, buffer);
                        snapshot = snapshot.add_application_state(
                            format!("key_{}", i),
                            format!("value_{}", i),
                        );
                    }
                    
                    // Serialize
                    let serialized = serialization_service.serialize(black_box(&snapshot)).await
                        .expect("Serialization failed");
                    
                    // Encrypt
                    let encrypted = encryption_service.encrypt(black_box(&serialized), black_box(&key))
                        .expect("Encryption failed");
                    
                    // Create migration request
                    let migration_request = rpcnet::migration::models::MigrationRequest::new(
                        format!("workflow-migration-{}", num_streams),
                        format!("workflow-conn-{}", num_streams),
                        "source-worker".to_string(),
                        "target-worker".to_string(),
                        encrypted.clone(),
                        SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
                    );
                    
                    // Generate token
                    let token = token_manager.generate_token(black_box(&migration_request))
                        .expect("Token generation failed");
                    
                    // Validate token
                    let is_valid = token_manager.validate_token(black_box(&token), black_box(&migration_request))
                        .expect("Token validation failed");
                    assert!(is_valid);
                    
                    // Decrypt and deserialize on target
                    let decrypted = encryption_service.decrypt(black_box(&encrypted), black_box(&key))
                        .expect("Decryption failed");
                    let _restored: ConnectionStateSnapshot = serialization_service.deserialize(black_box(&decrypted)).await
                        .expect("Deserialization failed");
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark memory usage during large state migrations
fn memory_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("migration_memory");
    group.sample_size(5); // Fewer samples for memory-intensive test
    
    let serialization_service = SerializationService::new(SerializationConfig {
        compression: CompressionMethod::Gzip,
        chunk_size: 64 * 1024,
    });
    
    // Test very large state snapshots
    let buffer_sizes = vec![10_000, 50_000, 100_000]; // Bytes per buffer
    let num_buffers = 100;
    
    for buffer_size in buffer_sizes {
        group.bench_with_input(
            BenchmarkId::new("large_state_serialization", buffer_size),
            &buffer_size,
            |b, &buffer_size| {
                b.to_async(&rt).iter(|| async {
                    let mut snapshot = ConnectionStateSnapshot::new(
                        format!("memory-test-{}", buffer_size),
                        SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
                    );
                    
                    for i in 0..num_buffers {
                        let buffer = vec![i as u8; buffer_size];
                        snapshot = snapshot.add_stream_buffer(i as u64, buffer);
                    }
                    
                    let _serialized = serialization_service.serialize(black_box(&snapshot)).await
                        .expect("Large state serialization failed");
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent migration operations
fn concurrency_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("migration_concurrency");
    group.sample_size(10);
    
    let token_manager = MigrationTokenManager::new(TokenValidationConfig {
        max_requests_per_second: 10000, // High limit for concurrency test
        token_expiry_seconds: 3600,
        secret_key: b"concurrency_benchmark_secret_32b".to_vec(),
    });
    
    let concurrent_operations = vec![10, 50, 100];
    
    for num_ops in concurrent_operations {
        group.bench_with_input(
            BenchmarkId::new("concurrent_token_operations", num_ops),
            &num_ops,
            |b, &num_ops| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();
                    
                    for i in 0..num_ops {
                        let token_manager_clone = token_manager.clone();
                        let handle = tokio::spawn(async move {
                            let migration_request = rpcnet::migration::models::MigrationRequest::new(
                                format!("concurrent-migration-{}", i),
                                format!("concurrent-conn-{}", i),
                                "source-worker".to_string(),
                                "target-worker".to_string(),
                                vec![i as u8; 100],
                                SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
                            );
                            
                            let token = token_manager_clone.generate_token(&migration_request)
                                .expect("Concurrent token generation failed");
                            
                            let is_valid = token_manager_clone.validate_token(&token, &migration_request)
                                .expect("Concurrent token validation failed");
                            
                            assert!(is_valid);
                        });
                        handles.push(handle);
                    }
                    
                    futures::future::join_all(handles).await;
                })
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    migration_benches,
    encryption_benchmark,
    serialization_benchmark,
    token_benchmark,
    complete_workflow_benchmark,
    memory_benchmark,
    concurrency_benchmark
);
criterion_main!(migration_benches);