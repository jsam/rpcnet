//! Python wrapper for RpcServer

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use crate::RpcServer;
use super::{config::PyRpcConfig, error::to_py_err};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Python wrapper for RPC server
///
/// This server handles incoming RPC requests over QUIC+TLS.
/// Handlers are Python async functions that process requests.
#[pyclass(name = "RpcServer")]
pub struct PyRpcServer {
    server: Arc<Mutex<RpcServer>>,
}

#[pymethods]
impl PyRpcServer {
    /// Create a new RPC server
    ///
    /// Args:
    ///     config: RpcConfig object with TLS settings and bind address
    ///
    /// Returns:
    ///     RpcServer: New server instance
    ///
    /// Example:
    ///     >>> config = RpcConfig(
    ///     ...     cert_path="certs/cert.pem",
    ///     ...     bind_addr="127.0.0.1:8080",
    ///     ...     key_path="certs/key.pem",
    ///     ... )
    ///     >>> server = RpcServer(config)
    #[new]
    fn new(config: &PyRpcConfig) -> PyResult<Self> {
        let server = RpcServer::new(config.inner.clone());
        Ok(PyRpcServer {
            server: Arc::new(Mutex::new(server)),
        })
    }

    /// Register an RPC method handler (async)
    ///
    /// The handler must be an async Python function that takes bytes
    /// and returns bytes.
    ///
    /// Args:
    ///     method_name: Name of the RPC method
    ///     handler: Async Python function (bytes) -> bytes
    ///
    /// Example:
    ///     >>> async def handle_add(request_bytes):
    ///     ...     request = json.loads(request_bytes.decode())
    ///     ...     result = request["a"] + request["b"]
    ///     ...     return json.dumps({"result": result}).encode()
    ///     >>> await server.register("add", handle_add)
    fn register<'py>(
        &self,
        py: Python<'py>,
        method_name: String,
        handler: PyObject,
    ) -> PyResult<Bound<'py, PyAny>> {
        let server = self.server.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Wrap Python async function to be callable from Rust
            let handler_fn = move |params: Vec<u8>| {
                let handler = Python::with_gil(|py| handler.clone_ref(py));
                async move {
                    // Create coroutine and convert to Rust future in one step
                    let future = Python::with_gil(|py| -> Result<_, crate::RpcError> {
                        let params_bytes = PyBytes::new_bound(py, &params);

                        // Call Python async function
                        let coroutine = handler
                            .call1(py, (params_bytes,))
                            .map_err(|e| crate::RpcError::InternalError(format!("Failed to call handler: {}", e)))?;

                        // Convert Python coroutine to Rust future
                        pyo3_async_runtimes::tokio::into_future(coroutine.into_bound(py))
                            .map_err(|e| crate::RpcError::InternalError(format!("Failed to convert coroutine: {}", e)))
                    })?;

                    // Await the future properly (non-blocking)
                    let result_obj = future
                        .await
                        .map_err(|e| crate::RpcError::InternalError(format!("Handler failed: {}", e)))?;

                    // Extract bytes from result
                    Python::with_gil(|py| {
                        result_obj
                            .extract::<Vec<u8>>(py)
                            .map_err(|e| crate::RpcError::InternalError(format!("Handler must return bytes: {}", e)))
                    })
                }
            };

            // Register handler with RpcServer
            let server_guard = server.lock().await;
            server_guard.register(&method_name, handler_fn).await;
            Ok(())
        })
    }

    /// Start the RPC server (async, blocking until shutdown)
    ///
    /// This method will block until the server is shut down.
    /// Run it with asyncio.create_task() to run in background.
    ///
    /// Raises:
    ///     TlsError: If TLS setup fails
    ///     ConnectionError: If bind fails
    ///
    /// Example:
    ///     >>> await server.serve()  # Blocks until shutdown
    fn serve<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let server = self.server.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut server_guard = server.lock().await;
            let quic_server = server_guard.bind().map_err(to_py_err)?;
            server_guard.start(quic_server).await.map_err(to_py_err)?;
            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        "RpcServer(ready)".to_string()
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}
