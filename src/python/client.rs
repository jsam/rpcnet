//! Python wrapper for RpcClient

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use crate::RpcClient;
use super::{config::PyRpcConfig, error::to_py_err};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

/// Python wrapper for RPC client
///
/// This client provides async RPC calls to a remote server over QUIC+TLS.
/// All methods are async and integrate with Python's asyncio event loop.
#[pyclass(name = "RpcClient")]
pub struct PyRpcClient {
    client: Arc<RpcClient>,
}

#[pymethods]
impl PyRpcClient {
    /// Connect to an RPC server (async)
    ///
    /// Args:
    ///     addr: Server address (e.g., "127.0.0.1:8080")
    ///     config: RpcConfig object with TLS settings
    ///
    /// Returns:
    ///     RpcClient: Connected client instance
    ///
    /// Raises:
    ///     ConnectionError: If connection fails
    ///     ValueError: If address is invalid
    ///
    /// Example:
    ///     >>> config = RpcConfig(
    ///     ...     cert_path="certs/cert.pem",
    ///     ...     bind_addr="0.0.0.0:0",
    ///     ...     server_name="localhost"
    ///     ... )
    ///     >>> client = await RpcClient.connect("127.0.0.1:8080", config)
    #[staticmethod]
    fn connect<'py>(
        py: Python<'py>,
        addr: String,
        config: &PyRpcConfig,
    ) -> PyResult<Bound<'py, PyAny>> {
        let socket_addr = SocketAddr::from_str(&addr)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(
                format!("Invalid address '{}': {}", addr, e)
            ))?;

        let config = config.inner.clone();

        // Bridge Rust async (Tokio) to Python async (asyncio)
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let client = RpcClient::connect(socket_addr, config)
                .await
                .map_err(to_py_err)?;

            Ok(PyRpcClient { client: Arc::new(client) })
        })
    }

    /// Call an RPC method (async)
    ///
    /// Args:
    ///     method: Method name to call
    ///     params: Request data as bytes
    ///
    /// Returns:
    ///     bytes: Response data
    ///
    /// Raises:
    ///     TimeoutError: If request times out
    ///     ConnectionError: If connection is lost
    ///     RpcError: For other RPC errors
    ///
    /// Example:
    ///     >>> request = json.dumps({"a": 10, "b": 20}).encode()
    ///     >>> response = await client.call("add", request)
    ///     >>> result = json.loads(response.decode())
    fn call<'py>(
        &self,
        py: Python<'py>,
        method: String,
        params: Vec<u8>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.client.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let result = client
                .call(&method, params)
                .await
                .map_err(to_py_err)?;

            Ok(Python::with_gil(|py| PyBytes::new_bound(py, &result).into_py(py)))
        })
    }

    /// Call an RPC method with a custom timeout (async)
    ///
    /// Args:
    ///     method: Method name to call
    ///     params: Request data as bytes
    ///     timeout_secs: Timeout in seconds (overrides config timeout)
    ///
    /// Returns:
    ///     bytes: Response data
    ///
    /// Raises:
    ///     TimeoutError: If request times out
    ///     ConnectionError: If connection is lost
    ///     RpcError: For other RPC errors
    ///
    /// Example:
    ///     >>> request = b"..."
    ///     >>> response = await client.call_with_timeout("add", request, 5.0)
    fn call_with_timeout<'py>(
        &self,
        py: Python<'py>,
        method: String,
        params: Vec<u8>,
        timeout_secs: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.client.clone();
        let timeout_duration = std::time::Duration::from_secs_f64(timeout_secs);

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Wrap the call in a timeout
            let result = tokio::time::timeout(
                timeout_duration,
                client.call(&method, params)
            )
            .await
            .map_err(|_| to_py_err(crate::RpcError::Timeout))?
            .map_err(to_py_err)?;

            Ok(Python::with_gil(|py| PyBytes::new_bound(py, &result).into_py(py)))
        })
    }

    fn __repr__(&self) -> String {
        "RpcClient(connected)".to_string()
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}
