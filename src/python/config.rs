//! Python wrapper for RpcConfig

use pyo3::prelude::*;
use crate::RpcConfig;
use std::time::Duration;

/// Python wrapper for RPC configuration
#[pyclass(name = "RpcConfig")]
#[derive(Clone)]
pub struct PyRpcConfig {
    pub(crate) inner: RpcConfig,
}

#[pymethods]
impl PyRpcConfig {
    /// Create a new RPC configuration
    ///
    /// Args:
    ///     cert_path: Path to TLS certificate file
    ///     bind_addr: Address to bind to (e.g., "127.0.0.1:8080")
    ///     key_path: Optional path to private key file
    ///     server_name: Optional server name for TLS verification
    ///     timeout_secs: Optional timeout in seconds (default: 30)
    ///
    /// Returns:
    ///     RpcConfig: Configuration object
    ///
    /// Example:
    ///     >>> config = RpcConfig(
    ///     ...     cert_path="certs/cert.pem",
    ///     ...     bind_addr="127.0.0.1:8080",
    ///     ...     key_path="certs/key.pem",
    ///     ...     server_name="localhost",
    ///     ...     timeout_secs=10
    ///     ... )
    #[new]
    #[pyo3(signature = (cert_path, bind_addr, key_path=None, server_name=None, timeout_secs=None))]
    fn new(
        cert_path: String,
        bind_addr: String,
        key_path: Option<String>,
        server_name: Option<String>,
        timeout_secs: Option<u64>,
    ) -> PyResult<Self> {
        let mut config = RpcConfig::new(&cert_path, &bind_addr);

        if let Some(key) = key_path {
            config = config.with_key_path(&key);
        }

        if let Some(name) = server_name {
            config = config.with_server_name(&name);
        }

        if let Some(timeout) = timeout_secs {
            config = config.with_default_stream_timeout(Duration::from_secs(timeout));
        }

        Ok(PyRpcConfig { inner: config })
    }

    fn __repr__(&self) -> String {
        format!("RpcConfig(bind_address='{}')", self.inner.bind_address)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

#[cfg(all(test, feature = "python"))]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_minimal_config() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|_py| {
            let config = PyRpcConfig::new(
                "certs/test_cert.pem".to_string(),
                "127.0.0.1:8080".to_string(),
                None,
                None,
                None,
            ).unwrap();

            assert_eq!(config.inner.bind_address, "127.0.0.1:8080");
            assert!(config.inner.cert_path.to_str().unwrap().contains("test_cert.pem"));
        });
    }

    #[test]
    fn test_new_with_full_config() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|_py| {
            let config = PyRpcConfig::new(
                "certs/test_cert.pem".to_string(),
                "127.0.0.1:9090".to_string(),
                Some("certs/test_key.pem".to_string()),
                Some("localhost".to_string()),
                Some(60),
            ).unwrap();

            assert_eq!(config.inner.bind_address, "127.0.0.1:9090");
            assert!(config.inner.cert_path.to_str().unwrap().contains("test_cert.pem"));
            assert_eq!(config.inner.server_name, "localhost");
            assert_eq!(config.inner.default_stream_timeout, Duration::from_secs(60));
        });
    }

    #[test]
    fn test_with_custom_timeout() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|_py| {
            let config = PyRpcConfig::new(
                "certs/cert.pem".to_string(),
                "0.0.0.0:5000".to_string(),
                None,
                None,
                Some(120),
            ).unwrap();

            assert_eq!(config.inner.default_stream_timeout, Duration::from_secs(120));
        });
    }

    #[test]
    fn test_repr_format() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|_py| {
            let config = PyRpcConfig::new(
                "certs/cert.pem".to_string(),
                "192.168.1.1:7777".to_string(),
                None,
                None,
                None,
            ).unwrap();

            let repr = config.__repr__();
            assert_eq!(repr, "RpcConfig(bind_address='192.168.1.1:7777')");
        });
    }

    #[test]
    fn test_str_equals_repr() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|_py| {
            let config = PyRpcConfig::new(
                "certs/cert.pem".to_string(),
                "127.0.0.1:8000".to_string(),
                None,
                None,
                None,
            ).unwrap();

            assert_eq!(config.__str__(), config.__repr__());
        });
    }

    #[test]
    fn test_clone() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|_py| {
            let config1 = PyRpcConfig::new(
                "certs/cert.pem".to_string(),
                "127.0.0.1:8080".to_string(),
                Some("certs/key.pem".to_string()),
                Some("testserver".to_string()),
                Some(30),
            ).unwrap();

            let config2 = config1.clone();

            assert_eq!(config1.inner.bind_address, config2.inner.bind_address);
            assert_eq!(config1.inner.server_name, config2.inner.server_name);
            assert_eq!(config1.inner.default_stream_timeout, config2.inner.default_stream_timeout);
        });
    }

    #[test]
    fn test_with_server_name() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|_py| {
            let config = PyRpcConfig::new(
                "certs/cert.pem".to_string(),
                "127.0.0.1:8080".to_string(),
                None,
                Some("my-service.local".to_string()),
                None,
            ).unwrap();

            assert_eq!(config.inner.server_name, "my-service.local");
        });
    }

    #[test]
    fn test_with_key_path() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|_py| {
            let config = PyRpcConfig::new(
                "certs/cert.pem".to_string(),
                "127.0.0.1:8080".to_string(),
                Some("certs/private_key.pem".to_string()),
                None,
                None,
            ).unwrap();

            assert!(config.inner.key_path.is_some());
            assert!(config.inner.key_path.unwrap().to_str().unwrap().contains("private_key.pem"));
        });
    }
}
