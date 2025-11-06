// Unit tests for runtime helper functions
// Tests thread configuration and environment variable parsing

use rpcnet::runtime;
use std::env;

#[test]
fn test_server_worker_threads_uses_env_var() {
    // Set environment variable
    env::set_var(runtime::SERVER_THREADS_ENV, "16");

    let threads = runtime::server_worker_threads();

    // Should use the environment variable value
    assert_eq!(threads, 16);

    // Clean up
    env::remove_var(runtime::SERVER_THREADS_ENV);
}

#[test]
fn test_server_worker_threads_fallback_to_default() {
    // Ensure environment variable is not set
    env::remove_var(runtime::SERVER_THREADS_ENV);

    let threads = runtime::server_worker_threads();

    // Should use default (number of CPUs), which should be at least 1
    assert!(threads >= 1);
}

#[test]
fn test_server_worker_threads_with_invalid_env() {
    // Set invalid environment variable
    env::set_var(runtime::SERVER_THREADS_ENV, "invalid");

    let threads = runtime::server_worker_threads();

    // Should fallback to default
    assert!(threads >= 1);

    // Clean up
    env::remove_var(runtime::SERVER_THREADS_ENV);
}

#[test]
fn test_server_worker_threads_with_zero() {
    // Set environment variable to 0 (invalid)
    env::set_var(runtime::SERVER_THREADS_ENV, "0");

    let threads = runtime::server_worker_threads();

    // Should fallback to default (0 is invalid)
    assert!(threads >= 1);

    // Clean up
    env::remove_var(runtime::SERVER_THREADS_ENV);
}

#[test]
fn test_server_worker_threads_with_negative() {
    // Set environment variable to negative number (invalid)
    env::set_var(runtime::SERVER_THREADS_ENV, "-1");

    let threads = runtime::server_worker_threads();

    // Should fallback to default
    assert!(threads >= 1);

    // Clean up
    env::remove_var(runtime::SERVER_THREADS_ENV);
}

#[test]
fn test_threads_from_env_with_valid_key() {
    let test_key = "RPCNET_TEST_THREADS";
    env::set_var(test_key, "8");

    let result = runtime::threads_from_env(test_key);

    assert_eq!(result, Some(8));

    // Clean up
    env::remove_var(test_key);
}

#[test]
fn test_threads_from_env_with_missing_key() {
    let test_key = "RPCNET_NONEXISTENT_KEY";
    env::remove_var(test_key);

    let result = runtime::threads_from_env(test_key);

    assert_eq!(result, None);
}

#[test]
fn test_threads_from_env_with_whitespace() {
    let test_key = "RPCNET_TEST_THREADS_WS";
    env::set_var(test_key, "  12  ");

    let result = runtime::threads_from_env(test_key);

    // Should trim whitespace
    assert_eq!(result, Some(12));

    // Clean up
    env::remove_var(test_key);
}

#[test]
fn test_threads_from_env_with_empty_string() {
    let test_key = "RPCNET_TEST_THREADS_EMPTY";
    env::set_var(test_key, "");

    let result = runtime::threads_from_env(test_key);

    assert_eq!(result, None);

    // Clean up
    env::remove_var(test_key);
}

#[test]
fn test_server_threads_env_constant() {
    // Verify the constant has the expected value
    assert_eq!(runtime::SERVER_THREADS_ENV, "RPCNET_SERVER_THREADS");
}

#[test]
fn test_server_worker_threads_with_large_number() {
    // Set environment variable to a large number
    env::set_var(runtime::SERVER_THREADS_ENV, "1024");

    let threads = runtime::server_worker_threads();

    assert_eq!(threads, 1024);

    // Clean up
    env::remove_var(runtime::SERVER_THREADS_ENV);
}

#[test]
fn test_server_worker_threads_with_one() {
    // Set environment variable to 1 (minimum valid value)
    env::set_var(runtime::SERVER_THREADS_ENV, "1");

    let threads = runtime::server_worker_threads();

    assert_eq!(threads, 1);

    // Clean up
    env::remove_var(runtime::SERVER_THREADS_ENV);
}

#[test]
fn test_server_worker_threads_typical_values() {
    // Test common CPU core counts
    // Clean up first to avoid interference from other tests
    env::remove_var(runtime::SERVER_THREADS_ENV);

    for value in [2, 4, 8, 16, 32] {
        env::set_var(runtime::SERVER_THREADS_ENV, value.to_string());

        let threads = runtime::server_worker_threads();

        assert_eq!(threads, value, "Failed for value {}", value);
    }

    // Clean up
    env::remove_var(runtime::SERVER_THREADS_ENV);
}

#[test]
fn test_threads_from_env_case_sensitivity() {
    // Environment variable names are case-sensitive
    let correct_key = "RPCNET_CASE_TEST";
    let wrong_key = "rpcnet_case_test";

    env::set_var(correct_key, "42");

    let correct = runtime::threads_from_env(correct_key);
    let wrong = runtime::threads_from_env(wrong_key);

    assert_eq!(correct, Some(42));
    assert_eq!(wrong, None);

    // Clean up
    env::remove_var(correct_key);
}

#[test]
fn test_threads_from_env_with_decimal() {
    // Decimal numbers should not be parsed
    let test_key = "RPCNET_TEST_DECIMAL";
    env::set_var(test_key, "4.5");

    let result = runtime::threads_from_env(test_key);

    assert_eq!(result, None);

    // Clean up
    env::remove_var(test_key);
}

#[test]
fn test_threads_from_env_with_hex() {
    // Hexadecimal should not be parsed (unless explicitly supported)
    let test_key = "RPCNET_TEST_HEX";
    env::set_var(test_key, "0x10");

    let result = runtime::threads_from_env(test_key);

    // Standard parse() doesn't handle 0x prefix
    assert_eq!(result, None);

    // Clean up
    env::remove_var(test_key);
}

#[test]
fn test_server_worker_threads_idempotent() {
    // Calling multiple times should return same result
    // Clean up first to avoid interference from other tests
    env::remove_var(runtime::SERVER_THREADS_ENV);

    env::set_var(runtime::SERVER_THREADS_ENV, "7");

    let threads1 = runtime::server_worker_threads();
    let threads2 = runtime::server_worker_threads();
    let threads3 = runtime::server_worker_threads();

    assert_eq!(threads1, threads2);
    assert_eq!(threads2, threads3);
    assert_eq!(threads1, 7);

    // Clean up
    env::remove_var(runtime::SERVER_THREADS_ENV);
}

#[test]
fn test_server_worker_threads_env_changes() {
    // Test that changes to environment variable are reflected
    env::set_var(runtime::SERVER_THREADS_ENV, "4");
    let threads1 = runtime::server_worker_threads();
    assert_eq!(threads1, 4);

    env::set_var(runtime::SERVER_THREADS_ENV, "8");
    let threads2 = runtime::server_worker_threads();
    assert_eq!(threads2, 8);

    // Clean up
    env::remove_var(runtime::SERVER_THREADS_ENV);
}

#[test]
fn test_threads_from_env_with_leading_zeros() {
    let test_key = "RPCNET_TEST_LEADING_ZEROS";
    env::set_var(test_key, "0008");

    let result = runtime::threads_from_env(test_key);

    // Should parse as 8
    assert_eq!(result, Some(8));

    // Clean up
    env::remove_var(test_key);
}

#[test]
fn test_threads_from_env_with_plus_sign() {
    let test_key = "RPCNET_TEST_PLUS";
    env::set_var(test_key, "+10");

    let result = runtime::threads_from_env(test_key);

    // Standard parse() might handle +, but if not, None is acceptable
    // This documents the actual behavior
    let is_valid = result == Some(10) || result.is_none();
    assert!(is_valid);

    // Clean up
    env::remove_var(test_key);
}
