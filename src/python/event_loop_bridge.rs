//! Python event loop bridge for async handler execution
//!
//! This module provides a bridge between Tokio's async runtime and Python's asyncio event loop.
//! Instead of creating a new thread, we use Python's existing event loop.

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::sync::Arc;

/// Bridge between Tokio and Python's asyncio event loop
///
/// This struct provides a way to run Python async handlers from Rust's Tokio runtime
/// by utilizing Python's existing asyncio event loop.
pub struct PythonEventLoopBridge {
    /// Reference to Python's asyncio module
    asyncio: PyObject,
}

impl PythonEventLoopBridge {
    /// Create a new Python event loop bridge
    ///
    /// This uses the existing Python event loop or creates one if needed.
    pub fn new() -> PyResult<Arc<Self>> {
        Python::with_gil(|py| {
            // Import asyncio module
            let asyncio = py.import("asyncio")?;
            
            // Try to get the running event loop, or create a new one
            let event_loop = match asyncio.call_method0("get_running_loop") {
                Ok(loop_obj) => loop_obj,
                Err(_) => {
                    // No running loop, try to get the current event loop
                    match asyncio.call_method0("get_event_loop") {
                        Ok(loop_obj) => loop_obj,
                        Err(_) => {
                            // Create a new event loop
                            asyncio.call_method0("new_event_loop")?
                        }
                    }
                }
            };
            
            // Store the event loop in asyncio for later use
            asyncio.setattr("_rpcnet_event_loop", &event_loop)?;
            
            Ok(Arc::new(Self {
                asyncio: asyncio.into(),
            }))
        })
    }

    /// Call a Python async handler
    ///
    /// This runs the handler in Python's event loop context.
    pub async fn call_handler(
        &self,
        handler: PyObject,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, crate::RpcError> {
        // Use tokio's spawn_blocking to run Python code without blocking the async runtime
        let asyncio = Python::with_gil(|py| self.asyncio.clone_ref(py));
        let result = tokio::task::spawn_blocking(move || {
            Python::with_gil(|py| -> Result<Vec<u8>, crate::RpcError> {
                // Create bytes object from input data
                let params_bytes = PyBytes::new(py, &data);
                
                // Call the Python handler
                let coroutine = handler
                    .call1(py, (params_bytes,))
                    .map_err(|e| crate::RpcError::InternalError(format!("Failed to call handler: {}", e)))?;
                
                // Check if the result is a coroutine
                let inspect = py.import("inspect")
                    .map_err(|e| crate::RpcError::InternalError(format!("Failed to import inspect: {}", e)))?;
                
                let is_coroutine = inspect
                    .call_method1("iscoroutine", (&coroutine,))
                    .map_err(|e| crate::RpcError::InternalError(format!("Failed to check coroutine: {}", e)))?
                    .extract::<bool>()
                    .map_err(|e| crate::RpcError::InternalError(format!("Failed to extract bool: {}", e)))?;
                
                if is_coroutine {
                    // Use asyncio.run to execute the coroutine
                    // This creates a new event loop in the current thread if needed
                    let asyncio_mod = asyncio.bind(py);
                    let result = asyncio_mod
                        .call_method1("run", (coroutine,))
                        .map_err(|e| crate::RpcError::InternalError(format!("Failed to run coroutine: {}", e)))?;
                    
                    // Extract bytes from result
                    result
                        .extract::<Vec<u8>>()
                        .map_err(|e| crate::RpcError::InternalError(format!("Handler must return bytes: {}", e)))
                } else {
                    // If not a coroutine, it might be a regular function that returns bytes
                    coroutine
                        .extract::<Vec<u8>>(py)
                        .map_err(|e| crate::RpcError::InternalError(format!("Handler must return bytes: {}", e)))
                }
            })
        })
        .await
        .map_err(|e| crate::RpcError::InternalError(format!("Task join error: {}", e)))?;
        
        result
    }
}