//! Dedicated Python asyncio event loop executor
//!
//! This module provides a bridge between Tokio (Rust async) and asyncio (Python async)
//! by running a dedicated Python event loop in a separate thread.

use pyo3::prelude::*;
use pyo3::types::PyBytes;

#[cfg(test)]
use pyo3::ffi::c_str;

/// Executor that runs Python async handlers in a dedicated event loop thread
///
/// This creates a dedicated thread with a running Python asyncio event loop.
/// When handlers need to be executed, they are submitted to this event loop
/// using thread-safe synchronization.
#[derive(Clone)]
pub struct PythonEventLoopExecutor {
    _phantom: std::marker::PhantomData<()>,
}

impl PythonEventLoopExecutor {
    /// Create a new Python event loop executor
    ///
    /// Note: For now, this is a placeholder. The actual implementation
    /// will use `tokio::task::spawn_blocking` to execute Python handlers.
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            _phantom: std::marker::PhantomData,
        })
    }

    /// Execute a Python async handler
    ///
    /// This uses `spawn_blocking` to run the Python handler in a thread pool,
    /// where we can safely create and run a Python event loop.
    pub async fn execute_handler(
        &self,
        handler: PyObject,
        params: Vec<u8>,
    ) -> Result<Vec<u8>, crate::RpcError> {
        // Use spawn_blocking to execute Python code in a thread pool
        // This avoids the "no running event loop" error by creating
        // a fresh event loop in the blocking thread
        tokio::task::spawn_blocking(move || {
            Python::with_gil(|py| -> Result<Vec<u8>, crate::RpcError> {
                // Import asyncio
                let asyncio = py.import("asyncio").map_err(|e| {
                    crate::RpcError::InternalError(format!("Failed to import asyncio: {}", e))
                })?;

                // Convert params to PyBytes
                let params_bytes = PyBytes::new(py, &params);

                // Call the handler to get a coroutine
                let coroutine = handler.call1(py, (params_bytes,)).map_err(|e| {
                    crate::RpcError::InternalError(format!("Failed to call handler: {}", e))
                })?;

                // Run the coroutine using asyncio.run()
                // This creates a new event loop, runs the coroutine, and cleans up
                let result = asyncio
                    .call_method1("run", (coroutine,))
                    .map_err(|e| {
                        crate::RpcError::InternalError(format!("Handler execution failed: {}", e))
                    })?;

                // Extract bytes from result
                result.extract::<Vec<u8>>().map_err(|e| {
                    crate::RpcError::InternalError(format!(
                        "Handler must return bytes, got: {}",
                        e
                    ))
                })
            })
        })
        .await
        .map_err(|e| crate::RpcError::InternalError(format!("Task join error: {}", e)))?
    }
}

impl Default for PythonEventLoopExecutor {
    fn default() -> Self {
        Self::new().expect("Failed to create PythonEventLoopExecutor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_creation() {
        let executor = PythonEventLoopExecutor::new();
        assert!(executor.is_ok());
    }

    #[tokio::test]
    async fn test_execute_simple_handler() {
        let executor = PythonEventLoopExecutor::new().unwrap();

        // Create a simple Python async handler
        let handler = Python::with_gil(|py| -> PyResult<PyObject> {
            let code = c_str!(r#"
async def echo_handler(data):
    return data
echo_handler
"#);
            let locals = pyo3::types::PyDict::new(py);
            py.run(code, None, Some(&locals))?;
            Ok(locals.get_item("echo_handler")?.unwrap().into())
        })
        .unwrap();

        // Execute the handler
        let test_data = b"Hello, Python!".to_vec();
        let result = executor.execute_handler(handler, test_data.clone()).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_data);
    }

    #[tokio::test]
    async fn test_execute_handler_with_async_operation() {
        let executor = PythonEventLoopExecutor::new().unwrap();

        // Create a Python async handler that does some async work
        let handler = Python::with_gil(|py| -> PyResult<PyObject> {
            // Create a globals dict with asyncio available
            let globals = pyo3::types::PyDict::new(py);
            let builtins = py.import("builtins")?;
            globals.set_item("__builtins__", builtins)?;
            globals.set_item("asyncio", py.import("asyncio")?)?;

            let code = c_str!(r#"
async def async_handler(data):
    # Simulate async work
    await asyncio.sleep(0.01)
    # Transform the data
    return bytes([b + 1 for b in data])
"#);
            // Run code with globals that include asyncio
            py.run(code, Some(&globals), None)?;
            Ok(globals.get_item("async_handler")?.unwrap().into())
        })
        .unwrap();

        // Execute the handler
        let test_data = vec![1, 2, 3, 4, 5];
        let result = executor.execute_handler(handler, test_data.clone()).await;

        match &result {
            Ok(data) => println!("Success: got {} bytes", data.len()),
            Err(e) => println!("Error: {}", e),
        }

        assert!(result.is_ok(), "Handler execution failed: {:?}", result);
        let result_data = result.unwrap();
        assert_eq!(result_data, vec![2, 3, 4, 5, 6]);
    }
}
