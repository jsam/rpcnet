// Unit tests for core RPC types: RpcConfig, RpcRequest, RpcResponse, RpcError
// These tests cover builder methods, accessors, and Display implementations

use rpcnet::{RpcConfig, RpcError};
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_rpc_config_new() {
    let config = RpcConfig::new("certs/test.pem", "127.0.0.1:8080");

    assert_eq!(config.cert_path, PathBuf::from("certs/test.pem"));
    assert_eq!(config.bind_address, "127.0.0.1:8080");
    assert_eq!(config.server_name, "localhost");
    assert!(config.key_path.is_none());
    assert_eq!(config.keep_alive_interval, Some(Duration::from_secs(30)));
    assert_eq!(config.default_stream_timeout, Duration::from_secs(3));
}

#[test]
fn test_rpc_config_with_key_path() {
    let config = RpcConfig::new("certs/cert.pem", "127.0.0.1:8080").with_key_path("certs/key.pem");

    assert_eq!(config.key_path, Some(PathBuf::from("certs/key.pem")));
}

#[test]
fn test_rpc_config_with_server_name() {
    let config = RpcConfig::new("certs/cert.pem", "127.0.0.1:8080").with_server_name("example.com");

    assert_eq!(config.server_name, "example.com");
}

#[test]
fn test_rpc_config_with_keep_alive_interval() {
    let config = RpcConfig::new("certs/cert.pem", "127.0.0.1:8080")
        .with_keep_alive_interval(Duration::from_secs(60));

    assert_eq!(config.keep_alive_interval, Some(Duration::from_secs(60)));
}

#[test]
fn test_rpc_config_with_default_stream_timeout() {
    let config = RpcConfig::new("certs/cert.pem", "127.0.0.1:8080")
        .with_default_stream_timeout(Duration::from_secs(10));

    assert_eq!(config.default_stream_timeout, Duration::from_secs(10));
}

#[test]
fn test_rpc_config_chaining() {
    // Test that all builder methods can be chained
    let config = RpcConfig::new("certs/cert.pem", "127.0.0.1:8080")
        .with_key_path("certs/key.pem")
        .with_server_name("myserver.local")
        .with_keep_alive_interval(Duration::from_secs(45))
        .with_default_stream_timeout(Duration::from_secs(5));

    assert_eq!(config.cert_path, PathBuf::from("certs/cert.pem"));
    assert_eq!(config.key_path, Some(PathBuf::from("certs/key.pem")));
    assert_eq!(config.server_name, "myserver.local");
    assert_eq!(config.bind_address, "127.0.0.1:8080");
    assert_eq!(config.keep_alive_interval, Some(Duration::from_secs(45)));
    assert_eq!(config.default_stream_timeout, Duration::from_secs(5));
}

#[test]
fn test_rpc_config_bind_address_types() {
    // Test with &str
    let config1 = RpcConfig::new("cert.pem", "0.0.0.0:9000");
    assert_eq!(config1.bind_address, "0.0.0.0:9000");

    // Test with String
    let config2 = RpcConfig::new("cert.pem", String::from("192.168.1.1:3000"));
    assert_eq!(config2.bind_address, "192.168.1.1:3000");
}

#[test]
fn test_rpc_config_cert_path_types() {
    // Test with &str
    let config1 = RpcConfig::new("path/to/cert.pem", "127.0.0.1:8080");
    assert_eq!(config1.cert_path, PathBuf::from("path/to/cert.pem"));

    // Test with PathBuf
    let config2 = RpcConfig::new(PathBuf::from("/absolute/path/cert.pem"), "127.0.0.1:8080");
    assert_eq!(config2.cert_path, PathBuf::from("/absolute/path/cert.pem"));
}

#[test]
fn test_rpc_error_display_connection_error() {
    let err = RpcError::ConnectionError("Connection refused".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Connection error"));
    assert!(display.contains("Connection refused"));
}

#[test]
fn test_rpc_error_display_stream_error() {
    let err = RpcError::StreamError("Stream closed unexpectedly".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Stream error"));
    assert!(display.contains("Stream closed unexpectedly"));
}

#[test]
fn test_rpc_error_display_tls_error() {
    let err = RpcError::TlsError("Certificate validation failed".to_string());
    let display = format!("{}", err);
    assert!(display.contains("TLS error"));
    assert!(display.contains("Certificate validation failed"));
}

#[test]
fn test_rpc_error_display_timeout() {
    let err = RpcError::Timeout;
    let display = format!("{}", err);
    assert!(display.contains("timeout"));
}

#[test]
fn test_rpc_error_display_unknown_method() {
    let err = RpcError::UnknownMethod("nonexistent_method".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Unknown method"));
    assert!(display.contains("nonexistent_method"));
}

#[test]
fn test_rpc_error_display_config_error() {
    let err = RpcError::ConfigError("Invalid configuration".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Configuration error"));
    assert!(display.contains("Invalid configuration"));
}

#[test]
fn test_rpc_error_display_internal_error() {
    let err = RpcError::InternalError("Unexpected state".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Internal error"));
    assert!(display.contains("Unexpected state"));
}

#[test]
fn test_rpc_error_display_invalid_token() {
    let err = RpcError::InvalidToken;
    let display = format!("{}", err);
    assert!(display.contains("Invalid migration token"));
}

#[test]
fn test_rpc_error_display_migration_rejected() {
    let err = RpcError::MigrationRejected;
    let display = format!("{}", err);
    assert!(display.contains("Migration rejected"));
}

#[test]
fn test_rpc_error_debug() {
    // Test Debug impl for all error variants
    let errors = vec![
        RpcError::ConnectionError("test".into()),
        RpcError::StreamError("test".into()),
        RpcError::TlsError("test".into()),
        RpcError::Timeout,
        RpcError::UnknownMethod("test".into()),
        RpcError::ConfigError("test".into()),
        RpcError::InternalError("test".into()),
        RpcError::InvalidToken,
        RpcError::MigrationRejected,
    ];

    for err in errors {
        let debug_str = format!("{:?}", err);
        assert!(!debug_str.is_empty());
    }
}

#[test]
fn test_rpc_error_from_io_error() {
    // Test automatic conversion from std::io::Error
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let rpc_err: RpcError = io_err.into();

    match rpc_err {
        RpcError::IoError(_) => {} // Expected
        other => panic!("Expected IoError, got {:?}", other),
    }
}

#[test]
fn test_rpc_error_from_msgpack_error() {
    // Test automatic conversion from rmp_serde::Error
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct TestStruct {
        value: u32,
    }

    // Create a MessagePack error by deserializing invalid data
    let invalid_data = vec![0xFF, 0xFF, 0xFF, 0xFF];
    let result: Result<TestStruct, _> = rmp_serde::from_slice(&invalid_data);

    if let Err(msgpack_err) = result {
        let rpc_err: RpcError = msgpack_err.into();
        match rpc_err {
            RpcError::SerializationError(_) => {} // Expected
            other => panic!("Expected SerializationError, got {:?}", other),
        }
    }
}

#[test]
fn test_rpc_config_clone() {
    let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
        .with_key_path("key.pem")
        .with_server_name("test.local");

    let cloned = config.clone();

    assert_eq!(config.cert_path, cloned.cert_path);
    assert_eq!(config.key_path, cloned.key_path);
    assert_eq!(config.server_name, cloned.server_name);
    assert_eq!(config.bind_address, cloned.bind_address);
}

#[test]
fn test_rpc_config_debug() {
    let config = RpcConfig::new("cert.pem", "127.0.0.1:8080");
    let debug_str = format!("{:?}", config);

    assert!(debug_str.contains("RpcConfig"));
    assert!(debug_str.contains("cert.pem"));
    assert!(debug_str.contains("127.0.0.1:8080"));
}

#[test]
fn test_rpc_config_edge_cases() {
    // Test with empty strings (should still work, even if not practical)
    let config = RpcConfig::new("", "");
    assert_eq!(config.cert_path, PathBuf::from(""));
    assert_eq!(config.bind_address, "");

    // Test with very long strings
    let long_path = "a".repeat(1000);
    let config = RpcConfig::new(long_path.clone(), "127.0.0.1:8080");
    assert_eq!(config.cert_path, PathBuf::from(long_path));
}

#[test]
fn test_rpc_config_zero_timeout() {
    // Test with zero timeout (edge case)
    let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
        .with_default_stream_timeout(Duration::from_secs(0));

    assert_eq!(config.default_stream_timeout, Duration::from_secs(0));
}

#[test]
fn test_rpc_config_very_long_timeout() {
    // Test with very long timeout
    let long_timeout = Duration::from_secs(86400 * 365); // 1 year
    let config =
        RpcConfig::new("cert.pem", "127.0.0.1:8080").with_default_stream_timeout(long_timeout);

    assert_eq!(config.default_stream_timeout, long_timeout);
}
