// Blocking RPC client for Python that uses Rust event loop for better performance
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;

use super::config::PyRpcConfig;
use super::error::to_py_err;
use crate::RpcClient;

// Global runtime for all BlockingClient instances
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("Failed to create tokio runtime"))
}

/// A blocking RPC client that releases the GIL during operations
///
/// This client provides much better performance than the async client
/// by eliminating Python's asyncio overhead and managing the GIL efficiently.
#[pyclass(name = "BlockingClient", module = "_rpcnet")]
#[derive(Clone)]
pub struct PyBlockingClient {
    inner: Arc<RpcClient>,
}

#[pymethods]
impl PyBlockingClient {
    /// Connect to an RPC server (blocking)
    ///
    /// Args:
    ///     addr: Server address (e.g., "127.0.0.1:8080")
    ///     config: RPC configuration
    ///
    /// Returns:
    ///     BlockingClient: Connected client instance
    #[staticmethod]
    #[pyo3(signature = (addr, config))]
    fn connect(py: Python<'_>, addr: String, config: &PyRpcConfig) -> PyResult<Self> {
        let socket_addr = SocketAddr::from_str(&addr).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid address '{}': {}",
                addr, e
            ))
        })?;

        let rust_config = config.inner.clone();
        let runtime = get_runtime();

        // Release GIL during connection
        let client = py
            .allow_threads(|| {
                runtime.block_on(async { RpcClient::connect(socket_addr, rust_config).await })
            })
            .map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyConnectionError, _>(format!(
                    "Failed to connect: {}",
                    e
                ))
            })?;

        Ok(PyBlockingClient {
            inner: Arc::new(client),
        })
    }

    /// Make a blocking RPC call (releases GIL)
    ///
    /// Args:
    ///     method: RPC method name
    ///     data: Request data as bytes
    ///
    /// Returns:
    ///     bytes: Response data
    #[pyo3(signature = (method, data))]
    fn call(&self, py: Python<'_>, method: String, data: Vec<u8>) -> PyResult<PyObject> {
        let client = self.inner.clone();
        let runtime = get_runtime();

        // Release GIL for entire RPC operation
        let result = py
            .allow_threads(|| runtime.block_on(async move { client.call(&method, data).await }))
            .map_err(to_py_err)?;

        // Convert result back to Python bytes
        Ok(PyBytes::new(py, &result).into())
    }

    /// Make multiple RPC calls in a batch (releases GIL)
    ///
    /// This method provides excellent performance by processing multiple
    /// requests concurrently in Rust while releasing the Python GIL.
    ///
    /// Args:
    ///     requests: List of (method, data) tuples
    ///
    /// Returns:
    ///     List[bytes]: Response data for each request
    #[pyo3(signature = (requests))]
    fn call_batch(
        &self,
        py: Python<'_>,
        requests: Vec<(String, Vec<u8>)>,
    ) -> PyResult<Vec<PyObject>> {
        let client = self.inner.clone();
        let runtime = get_runtime();

        // Release GIL for all operations
        let results = py.allow_threads(|| {
            runtime.block_on(async move {
                let futures: Vec<_> = requests
                    .into_iter()
                    .map(|(method, data)| {
                        let client = client.clone();
                        async move { client.call(&method, data).await }
                    })
                    .collect();

                futures::future::join_all(futures).await
            })
        });

        // Convert results back to Python
        let mut py_results = Vec::new();
        for result in results {
            match result {
                Ok(data) => py_results.push(PyBytes::new(py, &data).into()),
                Err(e) => return Err(to_py_err(e)),
            }
        }

        Ok(py_results)
    }

    fn __repr__(&self) -> String {
        "BlockingClient(connected)".to_string()
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}
