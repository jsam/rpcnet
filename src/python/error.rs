//! Python exception types for RpcNet errors

use pyo3::prelude::*;
use pyo3::exceptions::PyException;
use crate::RpcError;

// Base RPC exception
pyo3::create_exception!(_rpcnet, PyRpcError, PyException, "Base exception for RPC errors");
pyo3::create_exception!(_rpcnet, PyConnectionError, PyRpcError, "Connection-related errors");
pyo3::create_exception!(_rpcnet, PyTimeoutError, PyRpcError, "Timeout errors");
pyo3::create_exception!(_rpcnet, PySerializationError, PyRpcError, "Serialization/deserialization errors");
pyo3::create_exception!(_rpcnet, PyTlsError, PyRpcError, "TLS/encryption errors");
pyo3::create_exception!(_rpcnet, PyStreamError, PyRpcError, "Streaming errors");
pyo3::create_exception!(_rpcnet, PyHandlerError, PyRpcError, "Handler execution errors");

/// Convert Rust RpcError to Python exception
pub fn to_py_err(err: RpcError) -> PyErr {
    match err {
        RpcError::ConnectionError(msg) => PyConnectionError::new_err(msg),
        RpcError::Timeout => PyTimeoutError::new_err("Request timeout"),
        RpcError::SerializationError(err) => PySerializationError::new_err(err.to_string()),
        RpcError::TlsError(msg) => PyTlsError::new_err(msg),
        RpcError::StreamError(msg) => PyStreamError::new_err(msg),
        _ => PyRpcError::new_err(err.to_string()),
    }
}

#[cfg(all(test, feature = "python"))]
mod tests {
    use super::*;

    #[test]
    fn test_to_py_err_connection_error() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let err = RpcError::ConnectionError("failed to connect".to_string());
            let py_err = to_py_err(err);

            // Check that the error type is PyConnectionError
            assert!(py_err.is_instance_of::<PyConnectionError>(py));

            // Check error message
            let err_str = format!("{}", py_err);
            assert!(err_str.contains("failed to connect"));
        });
    }

    #[test]
    fn test_to_py_err_timeout() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let err = RpcError::Timeout;
            let py_err = to_py_err(err);

            // Check that the error type is PyTimeoutError
            assert!(py_err.is_instance_of::<PyTimeoutError>(py));

            // Check error message
            let err_str = format!("{}", py_err);
            assert!(err_str.contains("Request timeout"));
        });
    }

    #[test]
    fn test_to_py_err_serialization_error() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let err = RpcError::SerializationError(
                bincode::ErrorKind::Custom("invalid data".to_string()).into()
            );
            let py_err = to_py_err(err);

            // Check that the error type is PySerializationError
            assert!(py_err.is_instance_of::<PySerializationError>(py));

            // Check error message contains the custom message
            let err_str = format!("{}", py_err);
            assert!(err_str.contains("invalid data"));
        });
    }

    #[test]
    fn test_to_py_err_tls_error() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let err = RpcError::TlsError("certificate validation failed".to_string());
            let py_err = to_py_err(err);

            // Check that the error type is PyTlsError
            assert!(py_err.is_instance_of::<PyTlsError>(py));

            // Check error message
            let err_str = format!("{}", py_err);
            assert!(err_str.contains("certificate validation failed"));
        });
    }

    #[test]
    fn test_to_py_err_stream_error() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let err = RpcError::StreamError("stream closed unexpectedly".to_string());
            let py_err = to_py_err(err);

            // Check that the error type is PyStreamError
            assert!(py_err.is_instance_of::<PyStreamError>(py));

            // Check error message
            let err_str = format!("{}", py_err);
            assert!(err_str.contains("stream closed unexpectedly"));
        });
    }

    #[test]
    fn test_to_py_err_config_error() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let err = RpcError::ConfigError("invalid configuration".to_string());
            let py_err = to_py_err(err);

            // Config errors fall back to base PyRpcError
            assert!(py_err.is_instance_of::<PyRpcError>(py));

            // Check error message
            let err_str = format!("{}", py_err);
            assert!(err_str.contains("invalid configuration"));
        });
    }

    #[test]
    fn test_to_py_err_internal_error() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let err = RpcError::InternalError("unexpected error".to_string());
            let py_err = to_py_err(err);

            // Internal errors fall back to base PyRpcError
            assert!(py_err.is_instance_of::<PyRpcError>(py));

            // Check error message
            let err_str = format!("{}", py_err);
            assert!(err_str.contains("unexpected error"));
        });
    }
}
