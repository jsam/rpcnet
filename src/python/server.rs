//! Python wrapper for RpcServer with true multi-process architecture

#![allow(clippy::useless_conversion)]

use super::{
    cluster::PyCluster, cluster::PyClusterConfig, cluster::PyQuicClient, config::PyRpcConfig,
    error::{cluster_err_to_py, to_py_err},
    worker_config::WorkerConfig,
    worker_manager::WorkerManager,
};
use crate::RpcServer;
use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Python wrapper for RPC server with true multi-process worker architecture
///
/// This server spawns multiple Python worker processes, each with its own GIL,
/// enabling true parallel execution of Python handlers.
#[pyclass(name = "RpcServer")]
pub struct PyRpcServer {
    server: Arc<Mutex<RpcServer>>,
    worker_manager: Arc<Mutex<Option<WorkerManager>>>,
    config: WorkerConfig,
    /// Handlers registered before serve() is called
    pending_handlers: Arc<Mutex<std::collections::HashMap<String, PyObject>>>,
}

#[pymethods]
impl PyRpcServer {
    /// Create a new RPC server
    ///
    /// The server automatically spawns worker processes equal to CPU count.
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
        let worker_config = WorkerConfig::new();
        worker_config.validate().map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Invalid worker config: {}", e))
        })?;

        let server = RpcServer::new(config.inner.clone());

        Ok(PyRpcServer {
            server: Arc::new(Mutex::new(server)),
            worker_manager: Arc::new(Mutex::new(None)),
            config: worker_config,
            pending_handlers: Arc::new(Mutex::new(std::collections::HashMap::new())),
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
        let pending_handlers = self.pending_handlers.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut handlers = pending_handlers.lock().await;
            handlers.insert(method_name, handler);
            Ok(())
        })
    }

    /// Register a server streaming RPC method handler (async generator)
    fn register_server_streaming<'py>(
        &self,
        py: Python<'py>,
        _method_name: String,
        _handler: PyObject,
    ) -> PyResult<Bound<'py, PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            Err::<(), _>(PyErr::new::<pyo3::exceptions::PyNotImplementedError, _>(
                "Streaming not yet supported in multi-process mode",
            ))
        })
    }

    /// Register a client streaming RPC method handler (N→1)
    fn register_client_streaming<'py>(
        &self,
        py: Python<'py>,
        _method_name: String,
        _handler: PyObject,
    ) -> PyResult<Bound<'py, PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            Err::<(), _>(PyErr::new::<pyo3::exceptions::PyNotImplementedError, _>(
                "Streaming not yet supported in multi-process mode",
            ))
        })
    }

    /// Register a bidirectional streaming RPC method handler (N→M)
    fn register_bidirectional<'py>(
        &self,
        py: Python<'py>,
        _method_name: String,
        _handler: PyObject,
    ) -> PyResult<Bound<'py, PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            Err::<(), _>(PyErr::new::<pyo3::exceptions::PyNotImplementedError, _>(
                "Streaming not yet supported in multi-process mode",
            ))
        })
    }

    /// Enable SWIM cluster functionality
    fn enable_cluster<'py>(
        &self,
        py: Python<'py>,
        config: PyClusterConfig,
        seeds: Vec<String>,
        quic_client: PyQuicClient,
    ) -> PyResult<Bound<'py, PyAny>> {
        let server = self.server.clone();
        
        future_into_py(py, async move {
            let server_guard = server.lock().await;
            
            // Parse seed addresses
            let seed_addrs: Result<Vec<std::net::SocketAddr>, _> = seeds
                .iter()
                .map(|s| s.parse())
                .collect();
            let seed_addrs = seed_addrs.map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Invalid seed address: {}", e))
            })?;
            
            // Enable cluster on the underlying Rust server
            server_guard
                .enable_cluster(config.inner.clone(), seed_addrs, quic_client.inner.clone())
                .await
                .map_err(cluster_err_to_py)?;
            
            Ok(())
        })
    }

    /// Get cluster handle if cluster is enabled
    fn cluster<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let server = self.server.clone();
        
        future_into_py(py, async move {
            let server_guard = server.lock().await;
            let cluster = server_guard.cluster().await;
            
            Ok::<Option<PyCluster>, PyErr>(cluster.map(|c| PyCluster { inner: c }))
        })
    }

    /// Bind the server to the configured address
    fn bind<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let server = self.server.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut server_guard = server.lock().await;
            let quic_server = server_guard.bind().map_err(to_py_err)?;
            let mut quic_server_guard = server_guard.quic_server.lock().await;
            *quic_server_guard = Some(quic_server);
            Ok(())
        })
    }

    /// Start the RPC server with worker processes
    ///
    /// This spawns worker processes, initializes handlers, and starts serving requests.
    ///
    /// Args:
    ///     graceful_shutdown_timeout: Timeout in seconds for graceful shutdown (default: 30)
    ///
    /// Example:
    ///     >>> await server.serve()
    #[pyo3(signature = (_graceful_shutdown_timeout=None))]
    fn serve<'py>(
        &self,
        py: Python<'py>,
        _graceful_shutdown_timeout: Option<u64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let server = self.server.clone();
        let worker_manager = self.worker_manager.clone();
        let config = self.config.clone();

        let pending_handlers = self.pending_handlers.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Create worker manager
            let mut manager = WorkerManager::new(config).map_err(|e| {
                pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to create workers: {}", e))
            })?;

            // Register handlers with worker manager
            let handlers_to_register = pending_handlers.lock().await;
            for (method_name, handler) in handlers_to_register.iter() {
                let handler_clone = Python::with_gil(|py| handler.clone_ref(py));
                manager.register_handler(method_name.clone(), handler_clone).await;
            }
            drop(handlers_to_register);

            // Initialize workers with handlers
            manager.initialize_workers().await.map_err(|e| {
                pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "Failed to initialize workers: {}",
                    e
                ))
            })?;

            // Register handler wrapper that routes to workers
            let manager_clone = Arc::new(Mutex::new(manager));
            let server_guard = server.lock().await;

            // Get all registered handler method names
            let method_names: Vec<String> = {
                let mgr = manager_clone.lock().await;
                let handlers_guard = mgr.handlers.lock().await;
                handlers_guard.keys().cloned().collect()
            };

            for method_name in method_names.iter() {
                let mgr = manager_clone.clone();
                let method = method_name.clone();

                let handler_fn = move |params: Vec<u8>| {
                    let mgr = mgr.clone();
                    let method = method.clone();
                    async move {
                        mgr.lock()
                            .await
                            .execute_handler(&method, params)
                            .await
                    }
                };

                server_guard.register(&method_name, handler_fn).await;
            }

            drop(server_guard);

            // Note: We don't store worker manager as it can't be cloned
            // It will be dropped after serve() completes

            // Bind and start server
            let mut server_guard = server.lock().await;
            let quic_server = {
                let mut quic_server_guard = server_guard.quic_server.lock().await;
                if quic_server_guard.is_some() {
                    quic_server_guard.take().unwrap()
                } else {
                    drop(quic_server_guard);
                    server_guard.bind().map_err(to_py_err)?
                }
            };

            server_guard.start(quic_server).await.map_err(to_py_err)?;
            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        format!("RpcServer(processes={})", self.config.num_processes)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}
