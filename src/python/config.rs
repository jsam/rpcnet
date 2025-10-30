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
