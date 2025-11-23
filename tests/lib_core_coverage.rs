//! Coverage tests for core lib.rs functionality

use rpcnet::{RpcConfig, RpcError};
use std::time::Duration;

// Config tests
#[test]
fn test_rpc_config_new() {
    let _config = RpcConfig::new("cert.pem", "127.0.0.1:8080");
}

#[test]
fn test_rpc_config_with_key_path() {
    let _config = RpcConfig::new("cert.pem", "0.0.0.0:0").with_key_path("key.pem");
}

#[test]
fn test_rpc_config_with_server_name() {
    let _config = RpcConfig::new("cert.pem", "0.0.0.0:0").with_server_name("testserver");
}

#[test]
fn test_rpc_config_with_keep_alive() {
    let _config =
        RpcConfig::new("cert.pem", "0.0.0.0:0").with_keep_alive_interval(Duration::from_secs(10));
}

#[test]
fn test_rpc_config_with_stream_timeout() {
    let _config = RpcConfig::new("cert.pem", "0.0.0.0:0")
        .with_default_stream_timeout(Duration::from_secs(60));
}

#[test]
fn test_rpc_config_builder_chain() {
    let _config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
        .with_key_path("key.pem")
        .with_server_name("testserver")
        .with_keep_alive_interval(Duration::from_secs(10))
        .with_default_stream_timeout(Duration::from_secs(60));
}

#[test]
fn test_rpc_config_clone() {
    let config1 = RpcConfig::new("cert.pem", "0.0.0.0:0");
    let _config2 = config1.clone();
}

#[test]
fn test_rpc_config_empty_server_name() {
    let _config = RpcConfig::new("cert.pem", "0.0.0.0:0").with_server_name("");
}

#[test]
fn test_rpc_config_long_server_name() {
    let long_name = "a".repeat(256);
    let _config = RpcConfig::new("cert.pem", "0.0.0.0:0").with_server_name(&long_name);
}

#[test]
fn test_rpc_config_ipv6_address() {
    let _config = RpcConfig::new("cert.pem", "[::1]:8080");
}

#[test]
fn test_rpc_config_different_bind_addresses() {
    for addr in ["0.0.0.0:0", "127.0.0.1:8080", "[::]:0", "[::1]:9000"] {
        let _config = RpcConfig::new("cert.pem", addr);
    }
}

// Error tests
#[test]
fn test_rpc_error_display() {
    let errors = vec![
        RpcError::ConnectionError("test".to_string()),
        RpcError::Timeout,
        RpcError::SerializationError("test".to_string()),
        RpcError::ConfigError("test".to_string()),
        RpcError::TlsError("test".to_string()),
        RpcError::InternalError("test".to_string()),
        RpcError::StreamError("test".to_string()),
        RpcError::UnknownMethod("test".to_string()),
    ];

    for error in errors {
        let _display = format!("{}", error);
        let _debug = format!("{:?}", error);
    }
}

#[test]
fn test_rpc_error_from_io_error() {
    use std::io;
    let io_err = io::Error::new(io::ErrorKind::ConnectionRefused, "refused");
    let _rpc_err: RpcError = io_err.into();
}
