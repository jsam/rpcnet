#![allow(clippy::all)]
#![allow(warnings)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::assertions_on_constants)]
use rpcnet::{RpcConfig, RpcError, RpcRequest, RpcResponse};
use std::path::PathBuf;
use std::time::Duration;

#[cfg(test)]
mod unit_tests {
    use super::*;

    // ==========================
    // RpcRequest Tests
    // ==========================
    #[test]
    fn test_rpc_request_new() {
        let request = RpcRequest::new(123, "test_method".to_string(), vec![1, 2, 3, 4]);

        assert_eq!(request.id(), 123);
        assert_eq!(request.method(), "test_method");
        assert_eq!(request.params(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_rpc_request_empty_params() {
        let request = RpcRequest::new(0, "empty".to_string(), vec![]);

        assert_eq!(request.id(), 0);
        assert_eq!(request.method(), "empty");
        assert_eq!(request.params(), &Vec::<u8>::new());
        assert!(request.params().is_empty());
    }

    #[test]
    fn test_rpc_request_large_id() {
        let large_id = u64::MAX;
        let request = RpcRequest::new(large_id, "test".to_string(), vec![]);

        assert_eq!(request.id(), large_id);
    }

    #[test]
    fn test_rpc_request_unicode_method() {
        let request = RpcRequest::new(1, "测试方法_ñ_emoji🚀".to_string(), vec![]);

        assert_eq!(request.method(), "测试方法_ñ_emoji🚀");
    }

    #[test]
    fn test_rpc_request_serialization() {
        let original = RpcRequest::new(42, "serialize_test".to_string(), vec![0xFF, 0x00, 0xAA]);

        let serialized = bincode::serialize(&original).unwrap();
        let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.id(), original.id());
        assert_eq!(deserialized.method(), original.method());
        assert_eq!(deserialized.params(), original.params());
    }

    // ==========================
    // RpcResponse Tests
    // ==========================
    #[test]
    fn test_rpc_response_new_success() {
        let response = RpcResponse::new(123, Some(vec![1, 2, 3]), None);

        assert_eq!(response.id(), 123);
        assert_eq!(response.result(), Some(&vec![1, 2, 3]));
        assert_eq!(response.error(), None);
    }

    #[test]
    fn test_rpc_response_new_error() {
        let response = RpcResponse::new(456, None, Some("Test error".to_string()));

        assert_eq!(response.id(), 456);
        assert_eq!(response.result(), None);
        assert_eq!(response.error(), Some(&"Test error".to_string()));
    }

    #[test]
    fn test_rpc_response_from_result_success() {
        let result: Result<Vec<u8>, RpcError> = Ok(vec![5, 10, 15]);
        let response = RpcResponse::from_result(789, result);

        assert_eq!(response.id(), 789);
        assert_eq!(response.result(), Some(&vec![5, 10, 15]));
        assert_eq!(response.error(), None);
    }

    #[test]
    fn test_rpc_response_from_result_error() {
        let result: Result<Vec<u8>, RpcError> = Err(RpcError::Timeout);
        let response = RpcResponse::from_result(111, result);

        assert_eq!(response.id(), 111);
        assert_eq!(response.result(), None);
        assert!(response.error().is_some());
        assert!(response.error().unwrap().contains("timeout"));
    }

    #[test]
    fn test_rpc_response_serialization() {
        let original = RpcResponse::new(999, Some(vec![0xDE, 0xAD, 0xBE, 0xEF]), None);

        let serialized = bincode::serialize(&original).unwrap();
        let deserialized: RpcResponse = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.id(), original.id());
        assert_eq!(deserialized.result(), original.result());
        assert_eq!(deserialized.error(), original.error());
    }

    #[test]
    fn test_rpc_response_from_various_errors() {
        let errors = vec![
            RpcError::ConnectionError("Connection failed".to_string()),
            RpcError::StreamError("Stream closed".to_string()),
            RpcError::TlsError("TLS handshake failed".to_string()),
            RpcError::Timeout,
            RpcError::UnknownMethod("unknown_method".to_string()),
            RpcError::ConfigError("Invalid config".to_string()),
        ];

        for (i, error) in errors.into_iter().enumerate() {
            let response = RpcResponse::from_result(i as u64, Err(error));
            assert_eq!(response.id(), i as u64);
            assert_eq!(response.result(), None);
            assert!(response.error().is_some());
        }
    }

    // ==========================
    // RpcConfig Tests
    // ==========================
    #[test]
    fn test_rpc_config_new() {
        let config = RpcConfig::new("test.pem", "127.0.0.1:8080");

        assert_eq!(config.cert_path, PathBuf::from("test.pem"));
        assert_eq!(config.bind_address, "127.0.0.1:8080");
        assert_eq!(config.server_name, "localhost");
        assert_eq!(config.key_path, None);
        assert_eq!(config.keep_alive_interval, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_rpc_config_builder_pattern() {
        let config = RpcConfig::new("cert.pem", "0.0.0.0:9000")
            .with_key_path("key.pem")
            .with_server_name("custom.server.com")
            .with_keep_alive_interval(Duration::from_secs(120));

        assert_eq!(config.cert_path, PathBuf::from("cert.pem"));
        assert_eq!(config.key_path, Some(PathBuf::from("key.pem")));
        assert_eq!(config.bind_address, "0.0.0.0:9000");
        assert_eq!(config.server_name, "custom.server.com");
        assert_eq!(config.keep_alive_interval, Some(Duration::from_secs(120)));
    }

    #[test]
    fn test_rpc_config_pathbuf_input() {
        let cert_path = PathBuf::from("/path/to/cert.pem");
        let key_path = PathBuf::from("/path/to/key.pem");

        let config = RpcConfig::new(&cert_path, "127.0.0.1:0").with_key_path(&key_path);

        assert_eq!(config.cert_path, cert_path);
        assert_eq!(config.key_path, Some(key_path));
    }

    #[test]
    fn test_rpc_config_clone() {
        let original = RpcConfig::new("test.pem", "127.0.0.1:8080")
            .with_key_path("key.pem")
            .with_server_name("test.server")
            .with_keep_alive_interval(Duration::from_secs(60));

        let cloned = original.clone();

        assert_eq!(original.cert_path, cloned.cert_path);
        assert_eq!(original.key_path, cloned.key_path);
        assert_eq!(original.bind_address, cloned.bind_address);
        assert_eq!(original.server_name, cloned.server_name);
        assert_eq!(original.keep_alive_interval, cloned.keep_alive_interval);
    }

    #[test]
    fn test_rpc_config_different_addresses() {
        let configs = vec![
            ("127.0.0.1:8080", "127.0.0.1:8080"),
            ("0.0.0.0:9000", "0.0.0.0:9000"),
            ("[::1]:8080", "[::1]:8080"),
            ("localhost:3000", "localhost:3000"),
        ];

        for (input, expected) in configs {
            let config = RpcConfig::new("test.pem", input);
            assert_eq!(config.bind_address, expected);
        }
    }

    #[test]
    fn test_rpc_config_no_keep_alive() {
        let config = RpcConfig::new("test.pem", "127.0.0.1:0")
            .with_keep_alive_interval(Duration::from_secs(0));

        assert_eq!(config.keep_alive_interval, Some(Duration::from_secs(0)));
    }

    // ==========================
    // RpcError Tests
    // ==========================
    #[test]
    fn test_rpc_error_display() {
        let errors = vec![
            (
                RpcError::ConnectionError("failed".to_string()),
                "Connection error: failed",
            ),
            (
                RpcError::StreamError("closed".to_string()),
                "Stream error: closed",
            ),
            (
                RpcError::TlsError("handshake".to_string()),
                "TLS error: handshake",
            ),
            (RpcError::Timeout, "Request timeout"),
            (
                RpcError::UnknownMethod("test".to_string()),
                "Unknown method: test",
            ),
            (
                RpcError::ConfigError("invalid".to_string()),
                "Configuration error: invalid",
            ),
        ];

        for (error, expected_message) in errors {
            assert_eq!(error.to_string(), expected_message);
        }
    }

    #[test]
    fn test_rpc_error_from_bincode() {
        let bincode_error = bincode::Error::from(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "test error",
        ));
        let rpc_error = RpcError::from(bincode_error);

        if let RpcError::SerializationError(_) = rpc_error {
            // Expected
        } else {
            panic!("Expected SerializationError variant");
        }
    }

    #[test]
    fn test_rpc_error_from_io() {
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let rpc_error = RpcError::from(io_error);

        if let RpcError::IoError(_) = rpc_error {
            // Expected
        } else {
            panic!("Expected IoError variant");
        }
    }

    #[test]
    fn test_rpc_error_debug_format() {
        let error = RpcError::ConnectionError("debug test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("ConnectionError"));
        assert!(debug_str.contains("debug test"));
    }

    // ==========================
    // Serialization Edge Cases
    // ==========================
    #[test]
    fn test_large_request_serialization() {
        let large_data = vec![0xFF; 1_000_000]; // 1MB of data
        let request = RpcRequest::new(1, "large_data".to_string(), large_data.clone());

        let serialized = bincode::serialize(&request).unwrap();
        let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.params().len(), 1_000_000);
        assert_eq!(deserialized.params(), &large_data);
    }

    #[test]
    fn test_empty_method_name() {
        let request = RpcRequest::new(1, "".to_string(), vec![]);
        assert_eq!(request.method(), "");

        // Should be serializable
        let serialized = bincode::serialize(&request).unwrap();
        let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.method(), "");
    }

    #[test]
    fn test_very_long_method_name() {
        let long_method = "a".repeat(10_000);
        let request = RpcRequest::new(1, long_method.clone(), vec![]);

        assert_eq!(request.method(), &long_method);

        // Should be serializable
        let serialized = bincode::serialize(&request).unwrap();
        let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.method(), &long_method);
    }

    #[test]
    fn test_response_with_large_error_message() {
        let large_error = "error ".repeat(10_000);
        let response = RpcResponse::new(1, None, Some(large_error.clone()));

        assert_eq!(response.error(), Some(&large_error));

        // Should be serializable
        let serialized = bincode::serialize(&response).unwrap();
        let deserialized: RpcResponse = bincode::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.error(), Some(&large_error));
    }

    // ==========================
    // Default Values and Constants
    // ==========================
    #[test]
    fn test_default_timeout_values() {
        // Test that the timeout constants are defined correctly
        // Note: When running with tarpaulin, the constant value may be 30s instead of 2s
        let timeout = rpcnet::DEFAULT_TIMEOUT;
        assert!(timeout == Duration::from_secs(2) || timeout == Duration::from_secs(30));
    }

    // ==========================
    // Edge Cases and Error Conditions
    // ==========================
    #[test]
    fn test_zero_id_request_response() {
        let request = RpcRequest::new(0, "zero_id".to_string(), vec![]);
        let response = RpcResponse::new(0, Some(vec![]), None);

        assert_eq!(request.id(), 0);
        assert_eq!(response.id(), 0);
    }

    #[test]
    fn test_max_id_values() {
        let max_id = u64::MAX;
        let request = RpcRequest::new(max_id, "max_id".to_string(), vec![]);
        let response = RpcResponse::new(max_id, Some(vec![]), None);

        assert_eq!(request.id(), max_id);
        assert_eq!(response.id(), max_id);
    }

    #[test]
    fn test_config_with_various_server_names() {
        let server_names = vec![
            "",
            "localhost",
            "server.example.com",
            "192.168.1.1",
            "[::1]",
            "very-long-hostname-that-might-exceed-normal-limits.subdomain.example.org",
            "🚀.example.com", // Unicode domain (may not be practical but should not crash)
        ];

        for server_name in server_names {
            let config = RpcConfig::new("test.pem", "127.0.0.1:0").with_server_name(server_name);
            assert_eq!(config.server_name, server_name);
        }
    }
}
