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
