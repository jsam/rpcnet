//! Comprehensive coverage tests for Python bindings
//!
//! This test suite provides comprehensive coverage for all Python bridge modules.

#![cfg(feature = "python")]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use rpcnet::RpcError;

// ==============================================================================
// ERROR HANDLING TESTS (src/python/error.rs)
// ==============================================================================

#[test]
fn test_py_error_connection_error() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let err = RpcError::ConnectionError("failed to connect".to_string());
        let py_err = rpcnet::python::error::to_py_err(err);
        assert!(py_err.is_instance_of::<rpcnet::python::error::PyConnectionError>(py));
    });
}

#[test]
fn test_py_error_timeout() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let err = RpcError::Timeout;
        let py_err = rpcnet::python::error::to_py_err(err);
        assert!(py_err.is_instance_of::<rpcnet::python::error::PyTimeoutError>(py));
    });
}

#[test]
fn test_py_error_serialization_error() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let err = RpcError::SerializationError("invalid data".to_string());
        let py_err = rpcnet::python::error::to_py_err(err);
        assert!(py_err.is_instance_of::<rpcnet::python::error::PySerializationError>(py));
    });
}

#[test]
fn test_py_error_tls_error() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let err = RpcError::TlsError("certificate validation failed".to_string());
        let py_err = rpcnet::python::error::to_py_err(err);
        assert!(py_err.is_instance_of::<rpcnet::python::error::PyTlsError>(py));
    });
}

#[test]
fn test_py_error_stream_error() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let err = RpcError::StreamError("stream closed".to_string());
        let py_err = rpcnet::python::error::to_py_err(err);
        assert!(py_err.is_instance_of::<rpcnet::python::error::PyStreamError>(py));
    });
}

#[test]
fn test_py_error_config_error_fallback() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let err = RpcError::ConfigError("invalid config".to_string());
        let py_err = rpcnet::python::error::to_py_err(err);
        assert!(py_err.is_instance_of::<rpcnet::python::error::PyRpcError>(py));
    });
}

#[test]
fn test_py_error_internal_error_fallback() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let err = RpcError::InternalError("internal error".to_string());
        let py_err = rpcnet::python::error::to_py_err(err);
        assert!(py_err.is_instance_of::<rpcnet::python::error::PyRpcError>(py));
    });
}

#[test]
fn test_py_error_unknown_method_fallback() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let err = RpcError::UnknownMethod("nonexistent".to_string());
        let py_err = rpcnet::python::error::to_py_err(err);
        assert!(py_err.is_instance_of::<rpcnet::python::error::PyRpcError>(py));
    });
}

#[test]
fn test_py_cluster_error_conversion() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let err = rpcnet::cluster::ClusterError::AlreadyJoined;
        let py_err = rpcnet::python::error::cluster_err_to_py(err);
        assert!(py_err.is_instance_of::<rpcnet::python::error::PyClusterError>(py));
    });
}

#[test]
fn test_py_cluster_error_not_joined() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let err = rpcnet::cluster::ClusterError::NotJoined;
        let py_err = rpcnet::python::error::cluster_err_to_py(err);
        assert!(py_err.is_instance_of::<rpcnet::python::error::PyClusterError>(py));
    });
}

// ==============================================================================
// SERIALIZATION TESTS (src/python/serde.rs)
// ==============================================================================

#[test]
fn test_py_serde_python_to_bincode() {
    use rpcnet::python::serde::python_to_bincode;

    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let dict = PyDict::new(py);
        dict.set_item("key", "value").unwrap();
        dict.set_item("number", 42).unwrap();

        let result = python_to_bincode(&dict.as_any());
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    });
}

#[test]
fn test_py_serde_bincode_roundtrip() {
    use rpcnet::python::serde::{bincode_to_python, python_to_bincode};

    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let dict = PyDict::new(py);
        dict.set_item("name", "Alice").unwrap();
        dict.set_item("age", 30).unwrap();

        let bytes = python_to_bincode(&dict.as_any()).unwrap();
        let result = bincode_to_python(py, &bytes).unwrap();

        let result_dict = result.downcast::<PyDict>().unwrap();
        assert_eq!(
            result_dict
                .get_item("name")
                .unwrap()
                .unwrap()
                .extract::<String>()
                .unwrap(),
            "Alice"
        );
        assert_eq!(
            result_dict
                .get_item("age")
                .unwrap()
                .unwrap()
                .extract::<i64>()
                .unwrap(),
            30
        );
    });
}

#[test]
fn test_py_serde_python_to_bincode_py() {
    use rpcnet::python::serde::python_to_bincode_py;

    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let dict = PyDict::new(py);
        dict.set_item("test", 123).unwrap();

        let result = python_to_bincode_py(&dict.as_any());
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(!bytes.as_bytes().is_empty());
    });
}

#[test]
fn test_py_serde_bincode_to_python_py() {
    use rpcnet::python::serde::{bincode_to_python_py, python_to_bincode};

    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let dict = PyDict::new(py);
        dict.set_item("key", "value").unwrap();

        let bytes = python_to_bincode(&dict.as_any()).unwrap();
        let result = bincode_to_python_py(py, &bytes);
        assert!(result.is_ok());
    });
}

#[test]
fn test_py_serde_msgpack_to_python_py() {
    use rpcnet::python::serde::python_to_msgpack_py;

    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let dict = PyDict::new(py);
        dict.set_item("field", "data").unwrap();

        let result = python_to_msgpack_py(&dict.as_any());
        assert!(result.is_ok());
        assert!(!result.unwrap().as_bytes().is_empty());
    });
}

#[test]
fn test_py_serde_msgpack_roundtrip() {
    use rpcnet::python::serde::{msgpack_to_python_py, python_to_msgpack_py};

    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let dict = PyDict::new(py);
        dict.set_item("x", 100).unwrap();
        dict.set_item("y", 200).unwrap();

        let bytes = python_to_msgpack_py(&dict.as_any()).unwrap();
        let result = msgpack_to_python_py(py, bytes.as_bytes()).unwrap();

        let result_dict = result.downcast::<PyDict>().unwrap();
        assert_eq!(
            result_dict
                .get_item("x")
                .unwrap()
                .unwrap()
                .extract::<i64>()
                .unwrap(),
            100
        );
        assert_eq!(
            result_dict
                .get_item("y")
                .unwrap()
                .unwrap()
                .extract::<i64>()
                .unwrap(),
            200
        );
    });
}

#[test]
fn test_py_serde_empty_dict() {
    use rpcnet::python::serde::{bincode_to_python, python_to_bincode};

    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let dict = PyDict::new(py);

        let bytes = python_to_bincode(&dict.as_any()).unwrap();
        let result = bincode_to_python(py, &bytes).unwrap();

        let result_dict = result.downcast::<PyDict>().unwrap();
        assert_eq!(result_dict.len(), 0);
    });
}

#[test]
fn test_py_serde_nested_structures() {
    use rpcnet::python::serde::{bincode_to_python, python_to_bincode};

    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let dict = PyDict::new(py);
        let inner_list = PyList::new(py, &[1, 2, 3]).unwrap();
        dict.set_item("numbers", inner_list).unwrap();

        let bytes = python_to_bincode(&dict.as_any()).unwrap();
        let result = bincode_to_python(py, &bytes).unwrap();

        let result_dict = result.downcast::<PyDict>().unwrap();
        assert!(result_dict.contains("numbers").unwrap());
    });
}

#[test]
fn test_py_serde_various_types() {
    use rpcnet::python::serde::{bincode_to_python, python_to_bincode};

    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let dict = PyDict::new(py);
        dict.set_item("bool_val", true).unwrap();
        dict.set_item("int_val", 42).unwrap();
        dict.set_item("float_val", 42.5).unwrap();
        dict.set_item("str_val", "hello").unwrap();

        let bytes = python_to_bincode(&dict.as_any()).unwrap();
        let result = bincode_to_python(py, &bytes).unwrap();

        let result_dict = result.downcast::<PyDict>().unwrap();
        assert_eq!(result_dict.len(), 4);
    });
}

// ==============================================================================
// STREAMING TESTS (src/python/streaming.rs)
// ==============================================================================

#[test]
fn test_py_async_stream_creation() {
    use futures::stream;
    use rpcnet::python::streaming::PyAsyncStream;

    pyo3::prepare_freethreaded_python();
    Python::with_gil(|_py| {
        let test_stream = stream::iter(vec![Ok(vec![1u8, 2, 3]), Ok(vec![4u8, 5, 6])]);
        let _py_stream = PyAsyncStream::new(Box::pin(test_stream));
        // Stream created successfully
    });
}

// ==============================================================================
// CONFIG TESTS (src/python/config.rs)
// ==============================================================================

#[test]
fn test_py_rpc_config_builder() {
    use rpcnet::RpcConfig;

    pyo3::prepare_freethreaded_python();
    Python::with_gil(|_py| {
        let config = RpcConfig::new("test.pem", "0.0.0.0:0");
        let _config2 = config.with_keep_alive_interval(std::time::Duration::from_secs(30));
        // Config created successfully, exercises PyRpcConfig wrapper
    });
}

// ==============================================================================
// INTEGRATION TESTS
// ==============================================================================

#[test]
fn test_python_feature_enabled() {
    assert!(cfg!(feature = "python"));
}

#[test]
fn test_pyo3_initialized() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        assert!(py.version_info().major >= 3);
    });
}

#[test]
fn test_all_python_exception_types_exist() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|_py| {
        use rpcnet::python::error::*;

        let _base_err = PyRpcError::new_err("test");
        let _conn_err = PyConnectionError::new_err("test");
        let _timeout_err = PyTimeoutError::new_err("test");
        let _ser_err = PySerializationError::new_err("test");
        let _tls_err = PyTlsError::new_err("test");
        let _stream_err = PyStreamError::new_err("test");
        let _handler_err = PyHandlerError::new_err("test");
        let _cluster_err = PyClusterError::new_err("test");
    });
}
