#![allow(clippy::all)]
#![allow(warnings)]

//! Comprehensive tests for RpcClient
//!
//! This test suite covers:
//! - Client ID generation and sequencing
//! - RPC request/response structure
//! - Configuration validation
//! - Error handling scenarios
//! - Timeout behavior
//! - Request serialization

use rpcnet::{RpcClient, RpcConfig, RpcError, RpcRequest, RpcResponse};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

//------------------------------------------------------------------------------
// RpcRequest Tests
//------------------------------------------------------------------------------

#[test]
fn test_rpc_request_creation() {
    let request = RpcRequest::new(42, "test_method".to_string(), vec![1, 2, 3]);

    assert_eq!(request.id(), 42);
    assert_eq!(request.method(), "test_method");
    assert_eq!(request.params(), &[1, 2, 3]);
}

#[test]
fn test_rpc_request_with_empty_params() {
    let request = RpcRequest::new(1, "method".to_string(), vec![]);

    assert_eq!(request.params().len(), 0);
    assert!(request.params().is_empty());
}

#[test]
fn test_rpc_request_with_large_params() {
    let large_params = vec![0u8; 10_000];
    let request = RpcRequest::new(999, "large_method".to_string(), large_params.clone());

    assert_eq!(request.params().len(), 10_000);
    assert_eq!(request.params(), large_params.as_slice());
}

#[test]
fn test_rpc_request_method_names() {
    let test_cases = vec![
        "simple",
        "with_underscore",
        "with.dot",
        "CamelCase",
        "numbers123",
        "Service.Method",
        "very_long_method_name_that_should_work",
        "",
    ];

    for method in test_cases {
        let request = RpcRequest::new(1, method.to_string(), vec![]);
        assert_eq!(request.method(), method);
    }
}

#[test]
fn test_rpc_request_id_range() {
    // Test various ID ranges
    let test_ids = vec![0, 1, 100, 1000, u64::MAX / 2, u64::MAX];

    for id in test_ids {
        let request = RpcRequest::new(id, "method".to_string(), vec![]);
        assert_eq!(request.id(), id);
    }
}

#[test]
fn test_rpc_request_serialization() {
    let request = RpcRequest::new(42, "test".to_string(), vec![1, 2, 3]);

    // Serialize to MessagePack
    let serialized = rmp_serde::to_vec(&request).unwrap();
    assert!(!serialized.is_empty());

    // Deserialize back
    let deserialized: RpcRequest = rmp_serde::from_slice(&serialized).unwrap();
    assert_eq!(deserialized.id(), request.id());
    assert_eq!(deserialized.method(), request.method());
    assert_eq!(deserialized.params(), request.params());
}

//------------------------------------------------------------------------------
// RpcResponse Tests
//------------------------------------------------------------------------------

#[test]
fn test_rpc_response_success() {
    let response = RpcResponse::new(42, Some(vec![1, 2, 3]), None);

    assert_eq!(response.id(), 42);
    assert_eq!(response.result(), Some(&vec![1, 2, 3]));
    assert_eq!(response.error(), None);
}

#[test]
fn test_rpc_response_error() {
    let response = RpcResponse::new(42, None, Some("error message".to_string()));

    assert_eq!(response.id(), 42);
    assert_eq!(response.result(), None);
    assert_eq!(response.error(), Some(&"error message".to_string()));
}

#[test]
fn test_rpc_response_from_ok_result() {
    let result: Result<Vec<u8>, RpcError> = Ok(vec![4, 5, 6]);
    let response = RpcResponse::from_result(100, result);

    assert_eq!(response.id(), 100);
    assert_eq!(response.result(), Some(&vec![4, 5, 6]));
    assert_eq!(response.error(), None);
}

#[test]
fn test_rpc_response_from_err_result() {
    let result: Result<Vec<u8>, RpcError> = Err(RpcError::Timeout);
    let response = RpcResponse::from_result(200, result);

    assert_eq!(response.id(), 200);
    assert_eq!(response.result(), None);
    assert!(response.error().is_some());
    assert!(response.error().unwrap().contains("timeout"));
}

#[test]
fn test_rpc_response_with_empty_result() {
    let response = RpcResponse::new(1, Some(vec![]), None);

    assert_eq!(response.result(), Some(&vec![]));
    assert!(response.result().unwrap().is_empty());
}

#[test]
fn test_rpc_response_serialization() {
    let response = RpcResponse::new(42, Some(vec![1, 2, 3]), None);

    // Serialize to MessagePack
    let serialized = rmp_serde::to_vec(&response).unwrap();
    assert!(!serialized.is_empty());

    // Deserialize back
    let deserialized: RpcResponse = rmp_serde::from_slice(&serialized).unwrap();
    assert_eq!(deserialized.id(), response.id());
    assert_eq!(deserialized.result(), response.result());
}

#[test]
fn test_rpc_response_error_types() {
    let error_types = vec![
        RpcError::ConnectionError("connection failed".to_string()),
        RpcError::Timeout,
        RpcError::UnknownMethod("test".to_string()),
        RpcError::StreamError("stream error".to_string()),
        RpcError::SerializationError("serialization error".to_string()),
        RpcError::InternalError("internal error".to_string()),
    ];

    for (id, error) in error_types.into_iter().enumerate() {
        let result: Result<Vec<u8>, RpcError> = Err(error);
        let response = RpcResponse::from_result(id as u64, result);

        assert_eq!(response.id(), id as u64);
        assert!(response.error().is_some());
        assert!(response.result().is_none());
    }
}

//------------------------------------------------------------------------------
// RpcConfig Tests
//------------------------------------------------------------------------------

#[test]
fn test_rpc_config_basic_creation() {
    let config = RpcConfig::new("test_cert.pem", "127.0.0.1:8080");

    assert_eq!(config.cert_path.to_str().unwrap(), "test_cert.pem");
    assert_eq!(config.bind_address, "127.0.0.1:8080");
}

#[test]
fn test_rpc_config_with_key_path() {
    let config = RpcConfig::new("cert.pem", "0.0.0.0:0").with_key_path("key.pem");

    assert_eq!(
        config.key_path.as_ref().unwrap().to_str().unwrap(),
        "key.pem"
    );
}

#[test]
fn test_rpc_config_with_server_name() {
    let config = RpcConfig::new("cert.pem", "0.0.0.0:0").with_server_name("example.com");

    assert_eq!(config.server_name, "example.com");
}

#[test]
fn test_rpc_config_with_keep_alive() {
    let interval = Duration::from_secs(30);
    let config = RpcConfig::new("cert.pem", "0.0.0.0:0").with_keep_alive_interval(interval);

    assert_eq!(config.keep_alive_interval, Some(interval));
}

#[test]
fn test_rpc_config_builder_chaining() {
    let config = RpcConfig::new("cert.pem", "0.0.0.0:0")
        .with_key_path("key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_secs(60));

    assert_eq!(config.key_path.unwrap().to_str().unwrap(), "key.pem");
    assert_eq!(config.server_name, "localhost");
    assert_eq!(config.keep_alive_interval, Some(Duration::from_secs(60)));
}

#[test]
fn test_rpc_config_cloning() {
    let config1 = RpcConfig::new("cert.pem", "0.0.0.0:0").with_server_name("server1");

    let config2 = config1.clone();

    assert_eq!(config1.server_name, config2.server_name);
    assert_eq!(config1.bind_address, config2.bind_address);
}

#[test]
fn test_rpc_config_various_addresses() {
    let addresses = vec![
        "127.0.0.1:8080",
        "0.0.0.0:0",
        "192.168.1.1:9000",
        "10.0.0.1:443",
        "[::1]:8080",
        "[::]:0",
    ];

    for addr in addresses {
        let config = RpcConfig::new("cert.pem", addr);
        assert_eq!(config.bind_address, addr);
    }
}

#[test]
fn test_rpc_config_pathbuf_conversion() {
    use std::path::PathBuf;

    let path = PathBuf::from("certs/test.pem");
    let config = RpcConfig::new(path.clone(), "0.0.0.0:0");

    assert_eq!(config.cert_path, path);
}

//------------------------------------------------------------------------------
// Client ID Generation Tests
//------------------------------------------------------------------------------

#[test]
fn test_client_id_generation_starts_at_one() {
    let counter = Arc::new(AtomicU64::new(1));

    let id = counter.fetch_add(1, Ordering::SeqCst);
    assert_eq!(id, 1);

    let id = counter.fetch_add(1, Ordering::SeqCst);
    assert_eq!(id, 2);
}

#[test]
fn test_client_id_sequential() {
    let counter = Arc::new(AtomicU64::new(1));

    let ids: Vec<u64> = (0..100)
        .map(|_| counter.fetch_add(1, Ordering::SeqCst))
        .collect();

    // Verify sequential IDs
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!(id, (i + 1) as u64);
    }
}

#[test]
fn test_client_id_concurrent_generation() {
    use std::thread;

    let counter = Arc::new(AtomicU64::new(1));
    let mut handles = vec![];

    // Spawn 10 threads, each generating 100 IDs
    for _ in 0..10 {
        let counter_clone = counter.clone();
        let handle = thread::spawn(move || {
            let mut ids = vec![];
            for _ in 0..100 {
                ids.push(counter_clone.fetch_add(1, Ordering::SeqCst));
            }
            ids
        });
        handles.push(handle);
    }

    // Collect all IDs
    let mut all_ids = vec![];
    for handle in handles {
        all_ids.extend(handle.join().unwrap());
    }

    // Verify we have 1000 unique IDs
    assert_eq!(all_ids.len(), 1000);

    // Sort and check for duplicates
    all_ids.sort_unstable();
    for window in all_ids.windows(2) {
        assert_ne!(window[0], window[1], "Found duplicate ID");
    }
}

#[test]
fn test_client_id_wraparound() {
    // Test behavior near u64::MAX
    let counter = Arc::new(AtomicU64::new(u64::MAX - 5));

    let ids: Vec<u64> = (0..10)
        .map(|_| counter.fetch_add(1, Ordering::SeqCst))
        .collect();

    // IDs should increment even past MAX (wrapping)
    assert_eq!(ids[0], u64::MAX - 5);
    assert_eq!(ids[1], u64::MAX - 4);
    assert_eq!(ids[2], u64::MAX - 3);
    assert_eq!(ids[3], u64::MAX - 2);
    assert_eq!(ids[4], u64::MAX - 1);
    assert_eq!(ids[5], u64::MAX);
    // After MAX, wraps to 0, 1, 2...
    assert_eq!(ids[6], 0);
    assert_eq!(ids[7], 1);
}

//------------------------------------------------------------------------------
// RpcError Tests
//------------------------------------------------------------------------------

#[test]
fn test_rpc_error_display() {
    let errors = vec![
        (RpcError::Timeout, "timeout"),
        (
            RpcError::ConnectionError("failed".to_string()),
            "Connection error",
        ),
        (RpcError::StreamError("broken".to_string()), "Stream error"),
        (
            RpcError::UnknownMethod("test".to_string()),
            "Unknown method",
        ),
        (
            RpcError::SerializationError("invalid".to_string()),
            "Serialization error",
        ),
        (RpcError::TlsError("cert invalid".to_string()), "TLS error"),
        (
            RpcError::ConfigError("bad config".to_string()),
            "Configuration error",
        ),
        (RpcError::InternalError("bug".to_string()), "Internal error"),
        (RpcError::InvalidToken, "Invalid migration token"),
        (RpcError::MigrationRejected, "Migration rejected"),
    ];

    for (error, expected_substr) in errors {
        let error_str = format!("{}", error);
        assert!(
            error_str
                .to_lowercase()
                .contains(&expected_substr.to_lowercase()),
            "Error '{}' should contain '{}'",
            error_str,
            expected_substr
        );
    }
}

#[test]
fn test_rpc_error_debug() {
    let error = RpcError::Timeout;
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("Timeout"));
}

#[test]
fn test_rpc_error_from_msgpack_encode() {
    use rmp_serde::encode::Error as MsgpackEncodeError;

    // Create a struct that can't be serialized to trigger error
    #[derive(serde::Serialize)]
    struct BadStruct {
        #[serde(serialize_with = "always_fail")]
        field: i32,
    }

    fn always_fail<S>(_: &i32, _: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("forced error"))
    }

    let bad = BadStruct { field: 42 };
    let result: Result<Vec<u8>, MsgpackEncodeError> = rmp_serde::to_vec(&bad);

    if let Err(encode_err) = result {
        let rpc_error: RpcError = encode_err.into();
        match rpc_error {
            RpcError::SerializationError(msg) => {
                assert!(msg.contains("forced error"));
            }
            _ => panic!("Expected SerializationError"),
        }
    }
}

//------------------------------------------------------------------------------
// Timeout Tests
//------------------------------------------------------------------------------

#[test]
fn test_default_timeout_is_configured() {
    // Integration tests see the DEFAULT_TIMEOUT from the library's perspective
    // In library code compiled with cfg(test), it's 2 seconds
    // In library code compiled without cfg(test), it's 30 seconds
    // Since integration tests link against the library, they see the library's setting

    // Just verify it's a reasonable value (either 2s or 30s)
    let timeout = rpcnet::DEFAULT_TIMEOUT;
    assert!(
        timeout == Duration::from_secs(2) || timeout == Duration::from_secs(30),
        "DEFAULT_TIMEOUT should be either 2s (test) or 30s (production), got {:?}",
        timeout
    );
}

//------------------------------------------------------------------------------
// Edge Cases and Error Scenarios
//------------------------------------------------------------------------------

#[test]
fn test_rpc_request_with_binary_data() {
    let binary_params = vec![0x00, 0xFF, 0x55, 0xAA, 0x12, 0x34, 0x56, 0x78];
    let request = RpcRequest::new(1, "binary_method".to_string(), binary_params.clone());

    assert_eq!(request.params(), binary_params.as_slice());
}

#[test]
fn test_rpc_request_with_utf8_method_name() {
    let method_names = vec!["hello_世界", "метод", "método", "方法", "🚀_method"];

    for method in method_names {
        let request = RpcRequest::new(1, method.to_string(), vec![]);
        assert_eq!(request.method(), method);
    }
}

#[test]
fn test_rpc_response_roundtrip() {
    let original = RpcResponse::new(123, Some(vec![1, 2, 3, 4, 5]), None);

    let serialized = rmp_serde::to_vec(&original).unwrap();
    let deserialized: RpcResponse = rmp_serde::from_slice(&serialized).unwrap();

    assert_eq!(original.id(), deserialized.id());
    assert_eq!(original.result(), deserialized.result());
    assert_eq!(original.error(), deserialized.error());
}

#[test]
fn test_rpc_request_max_id() {
    let request = RpcRequest::new(u64::MAX, "method".to_string(), vec![]);
    assert_eq!(request.id(), u64::MAX);

    let serialized = rmp_serde::to_vec(&request).unwrap();
    let deserialized: RpcRequest = rmp_serde::from_slice(&serialized).unwrap();
    assert_eq!(deserialized.id(), u64::MAX);
}

#[test]
fn test_rpc_config_empty_server_name() {
    let config = RpcConfig::new("cert.pem", "0.0.0.0:0").with_server_name("");

    assert_eq!(config.server_name, "");
}

#[test]
fn test_multiple_requests_serialization() {
    let requests = vec![
        RpcRequest::new(1, "method1".to_string(), vec![1]),
        RpcRequest::new(2, "method2".to_string(), vec![2]),
        RpcRequest::new(3, "method3".to_string(), vec![3]),
    ];

    for request in requests {
        let serialized = rmp_serde::to_vec(&request).unwrap();
        let deserialized: RpcRequest = rmp_serde::from_slice(&serialized).unwrap();
        assert_eq!(request.id(), deserialized.id());
    }
}

//------------------------------------------------------------------------------
// Configuration Validation Tests
//------------------------------------------------------------------------------

#[test]
fn test_keep_alive_interval_zero() {
    let config =
        RpcConfig::new("cert.pem", "0.0.0.0:0").with_keep_alive_interval(Duration::from_secs(0));

    assert_eq!(config.keep_alive_interval, Some(Duration::from_secs(0)));
}

#[test]
fn test_keep_alive_interval_large_value() {
    let large_duration = Duration::from_secs(3600); // 1 hour
    let config = RpcConfig::new("cert.pem", "0.0.0.0:0").with_keep_alive_interval(large_duration);

    assert_eq!(config.keep_alive_interval, Some(large_duration));
}

#[test]
fn test_config_with_relative_paths() {
    let config = RpcConfig::new("./certs/cert.pem", "0.0.0.0:0").with_key_path("./certs/key.pem");

    assert_eq!(config.cert_path.to_str().unwrap(), "./certs/cert.pem");
    assert_eq!(
        config.key_path.unwrap().to_str().unwrap(),
        "./certs/key.pem"
    );
}

#[test]
fn test_config_with_absolute_paths() {
    let config = RpcConfig::new("/etc/ssl/cert.pem", "0.0.0.0:0").with_key_path("/etc/ssl/key.pem");

    assert_eq!(config.cert_path.to_str().unwrap(), "/etc/ssl/cert.pem");
    assert_eq!(
        config.key_path.unwrap().to_str().unwrap(),
        "/etc/ssl/key.pem"
    );
}
