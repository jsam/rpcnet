//! Python wrapper for RpcServer

#![allow(clippy::useless_conversion)]

use super::{config::PyRpcConfig, error::to_py_err, event_loop::PythonEventLoopExecutor};
use crate::RpcServer;
use pyo3::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Python wrapper for RPC server
///
/// This server handles incoming RPC requests over QUIC+TLS.
/// Handlers are Python async functions that process requests.
#[pyclass(name = "RpcServer")]
pub struct PyRpcServer {
    server: Arc<Mutex<RpcServer>>,
    /// Executor for running Python async handlers in a dedicated event loop
    executor: Arc<PythonEventLoopExecutor>,
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
        let executor = PythonEventLoopExecutor::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to create executor: {}", e)))?;

        Ok(PyRpcServer {
            server: Arc::new(Mutex::new(server)),
            executor: Arc::new(executor),
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
        let executor = self.executor.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Wrap Python async function to be callable from Rust
            // The executor handles running the Python handler in a dedicated event loop
            let handler_fn = move |params: Vec<u8>| {
                let handler = Python::with_gil(|py| handler.clone_ref(py));
                let executor = executor.clone();
                async move {
                    // Use the executor to run the Python handler
                    // This will execute it in a blocking thread pool with asyncio.run()
                    executor.execute_handler(handler, params).await
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
