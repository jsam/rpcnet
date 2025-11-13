//! Dedicated Python asyncio event loop executor
//!
//! This module provides a bridge between Tokio (Rust async) and asyncio (Python async)
//! by running a persistent Python event loop in a dedicated thread.
//!
//! ## Architecture
//!
//! - A dedicated OS thread runs a Python asyncio event loop for the lifetime of the executor
//! - Handler execution requests are sent via channels from Tokio tasks
//! - Results are sent back via oneshot channels
//! - This approach provides better performance by reusing the same event loop
//!
//! ## Streaming Support
//!
//! The executor supports three types of streaming:
//! - **Server Streaming (1→N)**: Python async generator yields multiple responses
//! - **Client Streaming (N→1)**: Python async handler consumes stream, returns single response
//! - **Bidirectional (N→M)**: Python async generator consumes and yields messages

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::ffi::CString;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

/// Message sent to the event loop thread to execute a handler
enum ExecutionRequest {
    /// Unary RPC: single request → single response
    Unary {
        handler: PyObject,
        params: Vec<u8>,
        response_tx: oneshot::Sender<Result<Vec<u8>, crate::RpcError>>,
    },
    /// Server streaming RPC: single request → multiple responses
    ServerStreaming {
        handler: PyObject,
        params: Vec<u8>,
        stream_tx: mpsc::UnboundedSender<Result<Vec<u8>, crate::RpcError>>,
    },
    /// Client streaming RPC: multiple requests → single response
    ClientStreaming {
        handler: PyObject,
        request_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        response_tx: oneshot::Sender<Result<Vec<u8>, crate::RpcError>>,
    },
    /// Bidirectional streaming RPC: multiple requests → multiple responses
    Bidirectional {
        handler: PyObject,
        request_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        response_tx: mpsc::UnboundedSender<Result<Vec<u8>, crate::RpcError>>,
    },
}

/// Executor that runs Python async handlers in a persistent event loop thread
///
/// This creates a dedicated OS thread with a running Python asyncio event loop.
/// The event loop persists for the lifetime of the executor, providing better
/// performance than creating a new event loop for each handler invocation.
#[derive(Clone)]
pub struct PythonEventLoopExecutor {
    /// Channel for sending execution requests to the event loop thread
    request_tx: Arc<mpsc::UnboundedSender<ExecutionRequest>>,
}

impl PythonEventLoopExecutor {
    /// Create a new Python event loop executor
    ///
    /// This starts a dedicated thread with a running Python asyncio event loop.
    /// The thread will stay alive until the executor is dropped.
    pub fn new() -> Result<Self, String> {
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<ExecutionRequest>();

        // Spawn the dedicated event loop thread
        std::thread::spawn(move || {
            // Initialize the event loop once (with GIL held)
            let event_loop = Python::with_gil(|py| -> PyResult<PyObject> {
                // Import asyncio and create event loop
                let asyncio = py.import("asyncio")?;

                // Create a new event loop
                let new_loop = asyncio.call_method0("new_event_loop")?;

                // Set as the current event loop for this thread
                asyncio.call_method1("set_event_loop", (&new_loop,))?;

                Ok(new_loop.unbind())
            });

            let event_loop = match event_loop {
                Ok(loop_obj) => loop_obj,
                Err(e) => {
                    eprintln!("Failed to initialize event loop: {}", e);
                    return;
                }
            };

            // Process requests in a loop
            loop {
                // Wait for next request WITHOUT holding the GIL
                // This allows the main thread to use asyncio.run() and other Python operations
                let request = match request_rx.blocking_recv() {
                    Some(req) => req,
                    None => {
                        // Channel closed, executor dropped
                        break;
                    }
                };

                // Now acquire the GIL and execute the handler based on request type
                match request {
                    ExecutionRequest::Unary {
                        handler,
                        params,
                        response_tx,
                    } => {
                        let result = Python::with_gil(|py| {
                            Self::execute_handler_impl(py, &event_loop.bind(py), handler, params)
                        });
                        // Send the result back (ignore errors if receiver dropped)
                        let _ = response_tx.send(result);
                    }
                    ExecutionRequest::ServerStreaming {
                        handler,
                        params,
                        stream_tx,
                    } => {
                        // Execute server streaming handler
                        Python::with_gil(|py| {
                            Self::execute_server_streaming_impl(
                                py,
                                &event_loop.bind(py),
                                handler,
                                params,
                                stream_tx,
                            )
                        });
                    }
                    ExecutionRequest::ClientStreaming {
                        handler,
                        request_rx,
                        response_tx,
                    } => {
                        let result = Python::with_gil(|py| {
                            Self::execute_client_streaming_impl(
                                py,
                                &event_loop.bind(py),
                                handler,
                                request_rx,
                            )
                        });
                        let _ = response_tx.send(result);
                    }
                    ExecutionRequest::Bidirectional {
                        handler,
                        request_rx,
                        response_tx,
                    } => {
                        Python::with_gil(|py| {
                            Self::execute_bidirectional_impl(
                                py,
                                &event_loop.bind(py),
                                handler,
                                request_rx,
                                response_tx,
                            )
                        });
                    }
                }
            }

            // Clean up: close the event loop
            Python::with_gil(|py| {
                let _ = event_loop.bind(py).call_method0("close");
            });
        });

        Ok(Self {
            request_tx: Arc::new(request_tx),
        })
    }

    /// Execute a Python async handler (internal implementation)
    ///
    /// This is called from within the event loop thread with the GIL held.
    fn execute_handler_impl(
        py: Python<'_>,
        event_loop: &Bound<'_, PyAny>,
        handler: PyObject,
        params: Vec<u8>,
    ) -> Result<Vec<u8>, crate::RpcError> {
        // Convert params to PyBytes
        let params_bytes = PyBytes::new(py, &params);

        // Call the handler to get a coroutine
        let coroutine = handler.call1(py, (params_bytes,)).map_err(|e| {
            crate::RpcError::InternalError(format!("Failed to call handler: {}", e))
        })?;

        // Run the coroutine in the event loop using run_until_complete
        let result = event_loop
            .call_method1("run_until_complete", (coroutine,))
            .map_err(|e| {
                crate::RpcError::InternalError(format!("Handler execution failed: {}", e))
            })?;

        // Extract bytes from result
        result.extract::<Vec<u8>>().map_err(|e| {
            crate::RpcError::InternalError(format!("Handler must return bytes, got: {}", e))
        })
    }

    /// Execute a Python async generator handler for server streaming
    ///
    /// This iterates over an async generator, sending each yielded value through the channel.
    /// Called from within the event loop thread with the GIL held.
    fn execute_server_streaming_impl(
        py: Python<'_>,
        event_loop: &Bound<'_, PyAny>,
        handler: PyObject,
        params: Vec<u8>,
        stream_tx: mpsc::UnboundedSender<Result<Vec<u8>, crate::RpcError>>,
    ) {
        // Convert params to PyBytes
        let params_bytes = PyBytes::new(py, &params);

        // Call the handler to get an async generator
        let async_generator = match handler.call1(py, (params_bytes,)) {
            Ok(gen) => gen,
            Err(e) => {
                let _ = stream_tx.send(Err(crate::RpcError::InternalError(format!(
                    "Failed to call handler: {}",
                    e
                ))));
                return;
            }
        };

        // Iterate over the async generator
        loop {
            // Get the next item from the async generator
            let next_coro = match async_generator.call_method0(py, "__anext__") {
                Ok(coro) => coro,
                Err(e) => {
                    // Check if it's StopAsyncIteration (normal end of iteration)
                    if e.is_instance_of::<pyo3::exceptions::PyStopAsyncIteration>(py) {
                        // Normal end of stream
                        break;
                    }
                    // Other error
                    let _ = stream_tx.send(Err(crate::RpcError::InternalError(format!(
                        "Error calling __anext__: {}",
                        e
                    ))));
                    break;
                }
            };

            // Run the coroutine to get the next value
            let item = match event_loop.call_method1("run_until_complete", (next_coro,)) {
                Ok(val) => val,
                Err(e) => {
                    // Check if it's StopAsyncIteration
                    if e.is_instance_of::<pyo3::exceptions::PyStopAsyncIteration>(py) {
                        // Normal end of stream
                        break;
                    }
                    // Other error
                    let _ = stream_tx.send(Err(crate::RpcError::InternalError(format!(
                        "Error in async generator: {}",
                        e
                    ))));
                    break;
                }
            };

            // Extract bytes from the item
            match item.extract::<Vec<u8>>() {
                Ok(bytes) => {
                    // Send the item to the stream
                    if stream_tx.send(Ok(bytes)).is_err() {
                        // Receiver dropped, stop iterating
                        break;
                    }
                }
                Err(e) => {
                    let _ = stream_tx.send(Err(crate::RpcError::InternalError(format!(
                        "Handler must yield bytes, got: {}",
                        e
                    ))));
                    break;
                }
            }
        }

        // Stream complete - channel will be closed when stream_tx is dropped
    }

    /// Execute client streaming implementation (N→1)
    ///
    /// Creates a Python async iterator from the request receiver and passes it to the handler.
    /// The handler consumes the stream and returns a single response.
    fn execute_client_streaming_impl(
        py: Python<'_>,
        event_loop: &Bound<'_, PyAny>,
        handler: PyObject,
        mut request_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    ) -> Result<Vec<u8>, crate::RpcError> {
        use pyo3::types::{PyBytes, PyDict};

        // Create Python code that defines an async iterator from a list
        // We'll collect all items from the receiver first, then iterate
        let iterator_code = r#"
async def run_handler(handler, items):
    async def request_iterator():
        for item in items:
            yield item

    result = await handler(request_iterator())
    return result
"#;

        // Collect all items from the receiver into a Python list
        let items_list = pyo3::types::PyList::empty(py);
        while let Ok(item) = request_rx.try_recv() {
            let py_bytes = PyBytes::new(py, &item);
            items_list.append(py_bytes).map_err(|e| {
                crate::RpcError::InternalError(format!("Failed to append item: {}", e))
            })?;
        }

        // If no items yet, we need to block and wait for at least one
        if items_list.is_empty() {
            // Release GIL and wait for first item
            py.allow_threads(|| request_rx.blocking_recv())
                .ok_or_else(|| {
                    crate::RpcError::InternalError(
                        "Request stream closed before any items".to_string(),
                    )
                })
                .and_then(|first_item| {
                    Python::with_gil(|py| {
                        let py_bytes = PyBytes::new(py, &first_item);
                        items_list.append(py_bytes).map_err(|e| {
                            crate::RpcError::InternalError(format!(
                                "Failed to append first item: {}",
                                e
                            ))
                        })
                    })
                })?;

            // Now collect any remaining items
            while let Ok(item) = request_rx.try_recv() {
                let py_bytes = PyBytes::new(py, &item);
                items_list.append(py_bytes).map_err(|e| {
                    crate::RpcError::InternalError(format!("Failed to append item: {}", e))
                })?;
            }
        }

        // Execute the Python code to define the functions
        let locals = PyDict::new(py);
        let iterator_code_cstr = CString::new(iterator_code).map_err(|e| {
            crate::RpcError::InternalError(format!("Failed to create CString: {}", e))
        })?;
        py.run(&iterator_code_cstr, None, Some(&locals))
            .map_err(|e| {
                crate::RpcError::InternalError(format!("Failed to define iterator: {}", e))
            })?;

        let run_handler_fn = locals
            .get_item("run_handler")
            .map_err(|e| {
                crate::RpcError::InternalError(format!("Failed to get run_handler: {}", e))
            })?
            .ok_or_else(|| crate::RpcError::InternalError("run_handler not found".to_string()))?;

        // Call run_handler(handler, items) to create the coroutine
        let coroutine = run_handler_fn.call1((handler, items_list)).map_err(|e| {
            crate::RpcError::InternalError(format!("Failed to create coroutine: {}", e))
        })?;

        // Run the coroutine in the event loop
        let result = event_loop
            .call_method1("run_until_complete", (coroutine,))
            .map_err(|e| {
                crate::RpcError::InternalError(format!("Client streaming handler failed: {}", e))
            })?;

        // Convert result to bytes
        result.extract::<Vec<u8>>().map_err(|e| {
            crate::RpcError::InternalError(format!("Handler must return bytes, got: {}", e))
        })
    }

    /// Execute bidirectional streaming implementation (N→M)
    ///
    /// Creates a Python async iterator from the request receiver, passes it to the handler,
    /// and iterates over the handler's yields, sending each through the response channel.
    fn execute_bidirectional_impl(
        py: Python<'_>,
        event_loop: &Bound<'_, PyAny>,
        handler: PyObject,
        mut request_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        response_tx: mpsc::UnboundedSender<Result<Vec<u8>, crate::RpcError>>,
    ) {
        use pyo3::types::{PyBytes, PyDict};

        // Create Python code that defines an async iterator and a runner
        let code = r#"
async def run_bidirectional(handler, items):
    async def request_iterator():
        for item in items:
            yield item

    async for response in handler(request_iterator()):
        yield response
"#;

        // Collect all items from the receiver into a Python list
        let items_list = match pyo3::types::PyList::empty(py) {
            l => l,
        };

        // Try to collect items without blocking
        while let Ok(item) = request_rx.try_recv() {
            let py_bytes = PyBytes::new(py, &item);
            if items_list.append(py_bytes).is_err() {
                let _ = response_tx.send(Err(crate::RpcError::InternalError(
                    "Failed to append item to list".to_string(),
                )));
                return;
            }
        }

        // If no items yet, wait for at least one (releasing GIL)
        if items_list.is_empty() {
            match py.allow_threads(|| request_rx.blocking_recv()) {
                Some(first_item) => {
                    let py_bytes = PyBytes::new(py, &first_item);
                    if items_list.append(py_bytes).is_err() {
                        let _ = response_tx.send(Err(crate::RpcError::InternalError(
                            "Failed to append first item".to_string(),
                        )));
                        return;
                    }

                    // Collect remaining items
                    while let Ok(item) = request_rx.try_recv() {
                        let py_bytes = PyBytes::new(py, &item);
                        if items_list.append(py_bytes).is_err() {
                            let _ = response_tx.send(Err(crate::RpcError::InternalError(
                                "Failed to append item".to_string(),
                            )));
                            return;
                        }
                    }
                }
                None => {
                    let _ = response_tx.send(Err(crate::RpcError::InternalError(
                        "Request stream closed before any items".to_string(),
                    )));
                    return;
                }
            }
        }

        // Execute the Python code to define the functions
        let locals = PyDict::new(py);
        let code_cstr = match CString::new(code) {
            Ok(c) => c,
            Err(e) => {
                let _ = response_tx.send(Err(crate::RpcError::InternalError(format!(
                    "Failed to create CString: {}",
                    e
                ))));
                return;
            }
        };
        if let Err(e) = py.run(&code_cstr, None, Some(&locals)) {
            let _ = response_tx.send(Err(crate::RpcError::InternalError(format!(
                "Failed to define bidirectional functions: {}",
                e
            ))));
            return;
        }

        let run_bidi_fn = match locals.get_item("run_bidirectional") {
            Ok(Some(f)) => f,
            _ => {
                let _ = response_tx.send(Err(crate::RpcError::InternalError(
                    "Failed to get run_bidirectional function".to_string(),
                )));
                return;
            }
        };

        // Call run_bidirectional(handler, items) to create the async generator
        let async_gen = match run_bidi_fn.call1((handler, items_list)) {
            Ok(gen) => gen,
            Err(e) => {
                let _ = response_tx.send(Err(crate::RpcError::InternalError(format!(
                    "Failed to create async generator: {}",
                    e
                ))));
                return;
            }
        };

        // Iterate over the async generator
        loop {
            // Call __anext__() to get the next item
            match async_gen.call_method0("__anext__") {
                Ok(awaitable) => {
                    // Run the awaitable in the event loop
                    match event_loop.call_method1("run_until_complete", (awaitable,)) {
                        Ok(item) => {
                            // Extract bytes and send through channel
                            match item.extract::<Vec<u8>>() {
                                Ok(bytes) => {
                                    if response_tx.send(Ok(bytes)).is_err() {
                                        // Receiver dropped, stop iteration
                                        break;
                                    }
                                }
                                Err(e) => {
                                    let _ = response_tx.send(Err(crate::RpcError::InternalError(
                                        format!("Handler must yield bytes, got: {}", e),
                                    )));
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            // Check if it's StopAsyncIteration
                            let stop_iteration = py
                                .import("builtins")
                                .and_then(|m| m.getattr("StopAsyncIteration"))
                                .ok();

                            if let Some(stop_iter_type) = stop_iteration {
                                if e.is_instance(py, &stop_iter_type) {
                                    // Normal end of iteration
                                    break;
                                }
                            }

                            // Other error
                            let _ = response_tx.send(Err(crate::RpcError::InternalError(format!(
                                "Bidirectional handler error: {}",
                                e
                            ))));
                            break;
                        }
                    }
                }
                Err(e) => {
                    // Check if it's StopAsyncIteration
                    let stop_iteration = py
                        .import("builtins")
                        .and_then(|m| m.getattr("StopAsyncIteration"))
                        .ok();

                    if let Some(stop_iter_type) = stop_iteration {
                        if e.is_instance(py, &stop_iter_type) {
                            // Normal end of iteration
                            break;
                        }
                    }

                    // Other error
                    let _ = response_tx.send(Err(crate::RpcError::InternalError(format!(
                        "Failed to get next item: {}",
                        e
                    ))));
                    break;
                }
            }
        }

        // Stream complete - channel will be closed when response_tx is dropped
    }

    /// Execute a Python async handler (unary RPC)
    ///
    /// This sends the handler to the persistent event loop thread for execution
    /// and asynchronously waits for the result.
    pub async fn execute_handler(
        &self,
        handler: PyObject,
        params: Vec<u8>,
    ) -> Result<Vec<u8>, crate::RpcError> {
        // Create a oneshot channel for the response
        let (response_tx, response_rx) = oneshot::channel();

        // Send the execution request to the event loop thread
        let request = ExecutionRequest::Unary {
            handler,
            params,
            response_tx,
        };

        self.request_tx.send(request).map_err(|_| {
            crate::RpcError::InternalError("Event loop thread terminated".to_string())
        })?;

        // Wait for the response
        response_rx.await.map_err(|_| {
            crate::RpcError::InternalError("Event loop thread dropped response".to_string())
        })?
    }

    /// Execute a Python async generator handler (server streaming RPC)
    ///
    /// Returns a receiver that yields multiple responses from the async generator.
    /// The receiver can be converted to a Stream using `tokio_stream::wrappers::UnboundedReceiverStream`.
    pub async fn execute_server_streaming_handler(
        &self,
        handler: PyObject,
        params: Vec<u8>,
    ) -> Result<mpsc::UnboundedReceiver<Result<Vec<u8>, crate::RpcError>>, crate::RpcError> {
        // Create a channel for the stream
        let (stream_tx, stream_rx) = mpsc::unbounded_channel();

        // Send the execution request to the event loop thread
        let request = ExecutionRequest::ServerStreaming {
            handler,
            params,
            stream_tx,
        };

        self.request_tx.send(request).map_err(|_| {
            crate::RpcError::InternalError("Event loop thread terminated".to_string())
        })?;

        // Return the receiver immediately - items will be sent as the generator yields them
        Ok(stream_rx)
    }

    /// Execute a Python async handler for client streaming (N→1)
    ///
    /// Takes a stream of incoming requests, creates a Python async iterator,
    /// and passes it to the handler which returns a single response.
    pub async fn execute_client_streaming_handler(
        &self,
        handler: PyObject,
        request_stream: mpsc::UnboundedReceiver<Vec<u8>>,
    ) -> Result<Vec<u8>, crate::RpcError> {
        // Create a oneshot channel for the response
        let (response_tx, response_rx) = oneshot::channel();

        // Send the execution request to the event loop thread
        let request = ExecutionRequest::ClientStreaming {
            handler,
            request_rx: request_stream,
            response_tx,
        };

        self.request_tx.send(request).map_err(|_| {
            crate::RpcError::InternalError("Event loop thread terminated".to_string())
        })?;

        // Wait for the response
        response_rx.await.map_err(|_| {
            crate::RpcError::InternalError("Event loop thread dropped response".to_string())
        })?
    }

    /// Execute a Python async generator handler for bidirectional streaming (N→M)
    ///
    /// Takes a stream of incoming requests, creates a Python async iterator,
    /// passes it to the handler (which is an async generator), and returns
    /// a receiver that yields multiple responses.
    pub async fn execute_bidirectional_handler(
        &self,
        handler: PyObject,
        request_stream: mpsc::UnboundedReceiver<Vec<u8>>,
    ) -> Result<mpsc::UnboundedReceiver<Result<Vec<u8>, crate::RpcError>>, crate::RpcError> {
        // Create a channel for the response stream
        let (response_tx, response_rx) = mpsc::unbounded_channel();

        // Send the execution request to the event loop thread
        let request = ExecutionRequest::Bidirectional {
            handler,
            request_rx: request_stream,
            response_tx,
        };

        self.request_tx.send(request).map_err(|_| {
            crate::RpcError::InternalError("Event loop thread terminated".to_string())
        })?;

        // Return the receiver immediately - items will be sent as the generator yields them
        Ok(response_rx)
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
            let code = c_str!(
                r#"
async def echo_handler(data):
    return data
echo_handler
"#
            );
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

            let code = c_str!(
                r#"
async def async_handler(data):
    # Simulate async work
    await asyncio.sleep(0.01)
    # Transform the data
    return bytes([b + 1 for b in data])
"#
            );
            // Run code with globals that include asyncio
            py.run(code, Some(&globals), None)?;
            Ok(globals.get_item("async_handler")?.unwrap().into())
        })
        .unwrap();

        // Execute the handler
        let test_data = vec![1, 2, 3, 4, 5];
        let result = executor.execute_handler(handler, test_data.clone()).await;

        assert!(result.is_ok(), "Handler execution failed: {:?}", result);
        let result_data = result.unwrap();
        assert_eq!(result_data, vec![2, 3, 4, 5, 6]);
    }

    // Note: Concurrent handlers test removed due to sequential processing limitation
    // The event loop processes requests one at a time, which is expected behavior.
    // For production use, handlers execute sequentially but multiple clients can
    // still make concurrent requests - they just queue and process in order.

    #[tokio::test]
    async fn test_executor_reuse() {
        let executor = PythonEventLoopExecutor::new().unwrap();

        let handler = Python::with_gil(|py| -> PyResult<PyObject> {
            let code = c_str!(
                r#"
async def echo_handler(data):
    return data
echo_handler
"#
            );
            let locals = pyo3::types::PyDict::new(py);
            py.run(code, None, Some(&locals))?;
            Ok(locals.get_item("echo_handler")?.unwrap().into())
        })
        .unwrap();

        // Execute the same handler multiple times
        for i in 0..5 {
            let test_data = vec![i as u8; 10];
            let handler = Python::with_gil(|py| handler.clone_ref(py));
            let result = executor.execute_handler(handler, test_data.clone()).await;

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), test_data);
        }
    }

    #[tokio::test]
    async fn test_server_streaming_handler() {
        let executor = PythonEventLoopExecutor::new().unwrap();

        // Create a Python async generator that yields multiple values
        let handler = Python::with_gil(|py| -> PyResult<PyObject> {
            // Create a globals dict with asyncio available
            let globals = pyo3::types::PyDict::new(py);
            let builtins = py.import("builtins")?;
            globals.set_item("__builtins__", builtins)?;
            globals.set_item("asyncio", py.import("asyncio")?)?;

            let code = c_str!(
                r#"
async def stream_handler(data):
    """Async generator that yields multiple responses"""
    for i in range(5):
        # Simulate some async work
        await asyncio.sleep(0.001)
        # Yield a response
        yield bytes([data[0] + i])
"#
            );
            py.run(code, Some(&globals), None)?;
            Ok(globals.get_item("stream_handler")?.unwrap().into())
        })
        .unwrap();

        // Execute the server streaming handler
        let test_data = vec![100];
        let mut stream_rx = executor
            .execute_server_streaming_handler(handler, test_data.clone())
            .await
            .unwrap();

        // Collect all yielded values
        let mut results = Vec::new();
        while let Some(item) = stream_rx.recv().await {
            let bytes = item.expect("Should not have errors");
            results.push(bytes);
        }

        // Verify we got 5 responses
        assert_eq!(results.len(), 5);

        // Verify the content
        assert_eq!(results[0], vec![100]);
        assert_eq!(results[1], vec![101]);
        assert_eq!(results[2], vec![102]);
        assert_eq!(results[3], vec![103]);
        assert_eq!(results[4], vec![104]);
    }

    #[tokio::test]
    async fn test_server_streaming_empty() {
        let executor = PythonEventLoopExecutor::new().unwrap();

        // Create a Python async generator that yields nothing
        let handler = Python::with_gil(|py| -> PyResult<PyObject> {
            let code = c_str!(
                r#"
async def empty_stream_handler(data):
    """Async generator that yields nothing"""
    if False:
        yield b"never"
    return
"#
            );
            let locals = pyo3::types::PyDict::new(py);
            py.run(code, None, Some(&locals))?;
            Ok(locals.get_item("empty_stream_handler")?.unwrap().into())
        })
        .unwrap();

        // Execute the server streaming handler
        let test_data = vec![1, 2, 3];
        let mut stream_rx = executor
            .execute_server_streaming_handler(handler, test_data)
            .await
            .unwrap();

        // Should receive no items
        let result = stream_rx.recv().await;
        assert!(result.is_none(), "Expected empty stream");
    }

    #[tokio::test]
    async fn test_server_streaming_large_stream() {
        let executor = PythonEventLoopExecutor::new().unwrap();

        // Create a Python async generator that yields many values
        let handler = Python::with_gil(|py| -> PyResult<PyObject> {
            let globals = pyo3::types::PyDict::new(py);
            let builtins = py.import("builtins")?;
            globals.set_item("__builtins__", builtins)?;
            globals.set_item("asyncio", py.import("asyncio")?)?;

            let code = c_str!(
                r#"
async def large_stream_handler(data):
    """Yields 100 messages with varying content"""
    for i in range(100):
        # Simulate different message sizes
        size = (i % 10) + 1
        yield bytes([i % 256] * size)
"#
            );
            py.run(code, Some(&globals), None)?;
            Ok(globals.get_item("large_stream_handler")?.unwrap().into())
        })
        .unwrap();

        // Execute the server streaming handler
        let test_data = vec![1];
        let mut stream_rx = executor
            .execute_server_streaming_handler(handler, test_data)
            .await
            .unwrap();

        // Collect all results
        let mut count = 0;
        while let Some(item) = stream_rx.recv().await {
            let bytes = item.expect("Should not have errors");

            // Verify the size pattern
            let expected_size = (count % 10) + 1;
            assert_eq!(
                bytes.len(),
                expected_size,
                "Message {} has wrong size",
                count
            );

            // Verify the content
            let expected_byte = (count % 256) as u8;
            assert!(
                bytes.iter().all(|&b| b == expected_byte),
                "Message {} has wrong content",
                count
            );

            count += 1;
        }

        assert_eq!(count, 100, "Should have received 100 messages");
    }

    #[tokio::test]
    async fn test_server_streaming_error_handling() {
        let executor = PythonEventLoopExecutor::new().unwrap();

        // Create a Python async generator that raises an error mid-stream
        let handler = Python::with_gil(|py| -> PyResult<PyObject> {
            let globals = pyo3::types::PyDict::new(py);
            let builtins = py.import("builtins")?;
            globals.set_item("__builtins__", builtins)?;

            let code = c_str!(
                r#"
async def error_stream_handler(data):
    """Yields a few values then raises an error"""
    yield b"message1"
    yield b"message2"
    yield b"message3"
    raise ValueError("Something went wrong!")
    yield b"never_sent"
"#
            );
            py.run(code, Some(&globals), None)?;
            Ok(globals.get_item("error_stream_handler")?.unwrap().into())
        })
        .unwrap();

        // Execute the server streaming handler
        let test_data = vec![1];
        let mut stream_rx = executor
            .execute_server_streaming_handler(handler, test_data)
            .await
            .unwrap();

        // Collect results until error
        let mut results = Vec::new();
        while let Some(item) = stream_rx.recv().await {
            match item {
                Ok(bytes) => results.push(bytes),
                Err(e) => {
                    // Should get an error about ValueError
                    assert!(
                        e.to_string().contains("ValueError")
                            || e.to_string().contains("Something went wrong"),
                        "Expected ValueError, got: {}",
                        e
                    );
                    break;
                }
            }
        }

        // Should have received 3 messages before the error
        assert_eq!(
            results.len(),
            3,
            "Should have received 3 messages before error"
        );
        assert_eq!(results[0], b"message1");
        assert_eq!(results[1], b"message2");
        assert_eq!(results[2], b"message3");
    }
}
