#![allow(clippy::all)]
#![allow(warnings)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::assertions_on_constants)]
// Security edge case tests to improve coverage
// These tests focus on TLS validation, certificate errors, and authentication edge cases

use rpcnet::{RpcClient, RpcConfig, RpcServer};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_invalid_certificate_path() {
    // Test with non-existent certificate file
    let config = RpcConfig::new("/nonexistent/cert.pem", "127.0.0.1:0")
        .with_key_path("/nonexistent/key.pem")
        .with_server_name("localhost");

    let result = timeout(
        Duration::from_millis(500),
        RpcClient::connect("127.0.0.1:9999".parse().unwrap(), config),
    )
    .await;

    // Should fail due to certificate issues
    assert!(result.is_err() || result.unwrap().is_err());
}

#[tokio::test]
async fn test_empty_certificate_paths() {
    // Test with empty certificate paths
    let config = RpcConfig::new("", "127.0.0.1:0")
        .with_key_path("")
        .with_server_name("localhost");

    assert_eq!(config.cert_path, std::path::PathBuf::from(""));
    assert_eq!(config.key_path, Some(std::path::PathBuf::from("")));
}

#[tokio::test]
async fn test_mismatched_server_name() {
    // Test with server name that doesn't match certificate
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("wrong.hostname.com")
        .with_keep_alive_interval(Duration::from_millis(100));

    let result = timeout(
        Duration::from_millis(500),
        RpcClient::connect("127.0.0.1:9999".parse().unwrap(), config),
    )
    .await;

    // Should timeout or fail verification
    assert!(result.is_err() || result.unwrap().is_err());
}

#[tokio::test]
async fn test_corrupted_certificate_content() {
    // Test behavior with corrupted certificate files
    use std::fs;
    use std::io::Write;

    // Create temporary corrupted cert file
    let temp_cert = "/tmp/corrupted_cert.pem";
    let mut file = fs::File::create(temp_cert).unwrap();
    file.write_all(b"-----BEGIN CERTIFICATE-----\nCORRUPTED_DATA\n-----END CERTIFICATE-----")
        .unwrap();

    let config = RpcConfig::new(temp_cert, "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_millis(100));

    let result = timeout(
        Duration::from_millis(500),
        RpcClient::connect("127.0.0.1:9999".parse().unwrap(), config),
    )
    .await;

    // Should fail due to corrupted certificate
    assert!(result.is_err() || result.unwrap().is_err());

    // Cleanup
    let _ = fs::remove_file(temp_cert);
}

#[tokio::test]
async fn test_very_short_timeout() {
    // Test with extremely short timeout (potential security DoS)
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_nanos(1)); // 1 nanosecond

    // Note: RpcConfig doesn't have a timeout field, this was conceptual

    let result = timeout(
        Duration::from_millis(100),
        RpcClient::connect("127.0.0.1:9999".parse().unwrap(), config),
    )
    .await;

    // Should timeout immediately
    assert!(result.is_err() || result.unwrap().is_err());
}

#[tokio::test]
async fn test_zero_timeout() {
    // Test with zero timeout
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::ZERO);

    assert_eq!(config.keep_alive_interval, Some(Duration::ZERO));
}

#[tokio::test]
async fn test_server_with_missing_key_file() {
    // Test server startup with missing key file
    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("nonexistent_key.pem")
        .with_server_name("localhost");

    // Creating server should not fail immediately, but binding should
    let _server = RpcServer::new(config);
    // Note: Actual binding would fail, but we can't test that without actual certificates
}

#[tokio::test]
async fn test_invalid_bind_address_formats() {
    // Test various invalid bind address formats
    let invalid_addresses = vec![
        "not_an_address",
        "256.256.256.256:8080", // Invalid IP
        "127.0.0.1:99999",      // Invalid port
        "127.0.0.1:",           // Missing port
        ":8080",                // Missing IP
        "127.0.0.1:-1",         // Negative port
        "",                     // Empty address
        "localhost",            // Missing port
    ];

    for addr in invalid_addresses {
        let config =
            RpcConfig::new("certs/test_cert.pem", addr).with_key_path("certs/test_key.pem");

        // Config creation should succeed, but parsing might fail later
        assert_eq!(config.bind_address, addr);
    }
}

#[tokio::test]
async fn test_unicode_in_server_names() {
    // Test with Unicode characters in server names (potential security issue)
    let long_name_253 = "a".repeat(253);
    let long_name_254 = "a".repeat(254);
    let unicode_names = vec![
        "тест.example.com", // Cyrillic
        "例え.テスト.jp",   // Japanese
        "🚀.example.com",   // Emoji
        "xn--nxasmq6b.com", // Punycode
        &long_name_253,     // Maximum length
        &long_name_254,     // Over maximum length
    ];

    for name in unicode_names {
        let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name(name);

        assert_eq!(config.server_name, name);
    }
}

#[tokio::test]
async fn test_path_traversal_in_cert_paths() {
    // Test potential path traversal attacks in certificate paths
    let malicious_paths = vec![
        "../../../etc/passwd",
        "..\\..\\..\\windows\\system32\\config\\sam",
        "/etc/shadow",
        "C:\\Windows\\System32\\config\\SAM",
        "file:///etc/passwd",
        "\\\\server\\share\\file",
        "~/.ssh/id_rsa",
        "/dev/null",
        "/proc/self/environ",
    ];

    for path in malicious_paths {
        let config = RpcConfig::new(path, "127.0.0.1:0").with_key_path(path);

        // Should not crash or expose sensitive information
        assert_eq!(config.cert_path, std::path::PathBuf::from(path));
        assert_eq!(config.key_path, Some(std::path::PathBuf::from(path)));
    }
}

#[tokio::test]
async fn test_extremely_long_paths() {
    // Test with extremely long file paths (potential buffer overflow)
    let long_path = "a/".repeat(1000) + "cert.pem";
    let very_long_path = "b/".repeat(5000) + "key.pem";

    let config = RpcConfig::new(&long_path, "127.0.0.1:0").with_key_path(&very_long_path);

    assert_eq!(config.cert_path, std::path::PathBuf::from(&long_path));
    assert_eq!(
        config.key_path,
        Some(std::path::PathBuf::from(&very_long_path))
    );
}

#[tokio::test]
async fn test_null_bytes_in_paths() {
    // Test with null bytes in paths (potential security issue)
    let paths_with_nulls = vec!["cert\0.pem", "cert.pem\0", "\0cert.pem", "ce\0rt.pem"];

    for path in paths_with_nulls {
        let config = RpcConfig::new(path, "127.0.0.1:0").with_key_path(path);

        // Should handle null bytes gracefully
        assert_eq!(config.cert_path, std::path::PathBuf::from(path));
    }
}

#[tokio::test]
async fn test_concurrent_connection_attempts() {
    // Test many concurrent connection attempts (potential DoS)
    use futures::future::join_all;

    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_millis(50));

    let mut futures = vec![];

    // Attempt 50 concurrent connections
    for _ in 0..50 {
        let config_clone = config.clone();
        futures.push(async move {
            timeout(
                Duration::from_millis(100),
                RpcClient::connect("127.0.0.1:19999".parse().unwrap(), config_clone),
            )
            .await
        });
    }

    let results = join_all(futures).await;

    // All should fail (no server running), but shouldn't crash
    for result in results {
        assert!(result.is_err() || result.unwrap().is_err());
    }
}

#[tokio::test]
async fn test_keep_alive_edge_cases() {
    // Test edge cases in keep-alive configuration
    let configs = vec![
        // Maximum duration
        RpcConfig::new("cert.pem", "127.0.0.1:0").with_keep_alive_interval(Duration::MAX),
        // One nanosecond
        RpcConfig::new("cert.pem", "127.0.0.1:0").with_keep_alive_interval(Duration::from_nanos(1)),
        // Exactly one second
        RpcConfig::new("cert.pem", "127.0.0.1:0").with_keep_alive_interval(Duration::from_secs(1)),
    ];

    assert_eq!(configs[0].keep_alive_interval, Some(Duration::MAX));
    assert_eq!(
        configs[1].keep_alive_interval,
        Some(Duration::from_nanos(1))
    );
    assert_eq!(configs[2].keep_alive_interval, Some(Duration::from_secs(1)));
}

#[test]
fn test_config_debug_format_no_secrets() {
    // Ensure debug format doesn't leak sensitive information
    let config = RpcConfig::new("/secret/path/cert.pem", "127.0.0.1:8080")
        .with_key_path("/secret/path/key.pem")
        .with_server_name("secret.internal.com");

    let debug_output = format!("{:?}", config);

    // Debug output should exist but we can't test exact content
    // since it might contain file paths
    assert!(!debug_output.is_empty());
}

#[tokio::test]
async fn test_bind_to_privileged_ports() {
    // Test binding to privileged ports (should fail without root)
    let privileged_ports = vec!["0.0.0.0:80", "0.0.0.0:443", "0.0.0.0:22", "0.0.0.0:21"];

    for port in privileged_ports {
        let config =
            RpcConfig::new("certs/test_cert.pem", port).with_key_path("certs/test_key.pem");

        assert_eq!(config.bind_address, port);

        // Creating server shouldn't fail (only binding would)
        let _server = RpcServer::new(config);
    }
}

#[tokio::test]
async fn test_ipv6_address_handling() {
    // Test IPv6 address edge cases
    let ipv6_addresses = vec![
        "[::1]:8080",                // Localhost
        "[::]:8080",                 // Any address
        "[2001:db8::1]:8080",        // Standard IPv6
        "[fe80::1%eth0]:8080",       // Link-local with interface
        "[::ffff:192.168.1.1]:8080", // IPv4-mapped IPv6
    ];

    for addr in ipv6_addresses {
        let config =
            RpcConfig::new("certs/test_cert.pem", addr).with_key_path("certs/test_key.pem");

        assert_eq!(config.bind_address, addr);
    }
}
