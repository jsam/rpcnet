//! Python wrappers for streaming RPC support
//!
//! This module provides Python bindings for streaming operations, allowing
//! Python code to consume Rust streams as async iterators.

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use futures::stream::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;
use super::error::to_py_err;

/// Python wrapper for async stream (async iterator)
///
/// This allows Python code to consume Rust streams using async for:
/// ```python
/// async for data in stream:
///     print(data)
/// ```
#[pyclass(name = "AsyncStream")]
pub struct PyAsyncStream {
    inner: Arc<Mutex<Pin<Box<dyn Stream<Item = Result<Vec<u8>, crate::RpcError>> + Send>>>>,
}

impl PyAsyncStream {
    /// Create a new AsyncStream from a Rust stream
    pub fn new(stream: Pin<Box<dyn Stream<Item = Result<Vec<u8>, crate::RpcError>> + Send>>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(stream)),
        }
    }
}

#[pymethods]
impl PyAsyncStream {
    /// Make this an async iterator
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Get next item from stream (async iterator protocol)
    fn __anext__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let stream = self.inner.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut stream_guard = stream.lock().await;

            match stream_guard.next().await {
                Some(Ok(data)) => {
                    // Return the data
                    Ok(Python::with_gil(|py| PyBytes::new_bound(py, &data).into_py(py)))
                }
                Some(Err(e)) => {
                    // Convert error and raise in Python
                    Err(to_py_err(e))
                }
                None => {
                    // End of stream - raise StopAsyncIteration
                    Err(pyo3::exceptions::PyStopAsyncIteration::new_err("Stream ended"))
                }
            }
        })
    }

    /// Collect all items from stream into a list
    ///
    /// Note: This will load all items into memory. Use iteration for large streams.
    fn collect<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let stream = self.inner.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut stream_guard = stream.lock().await;
            let mut items = Vec::new();

            while let Some(result) = stream_guard.next().await {
                match result {
                    Ok(data) => items.push(data),
                    Err(e) => return Err(to_py_err(e)),
                }
            }

            // Create Python list from collected items
            Ok(Python::with_gil(|py| {
                let py_list = pyo3::types::PyList::empty_bound(py);
                for item in items {
                    let _ = py_list.append(PyBytes::new_bound(py, &item));
                }
                py_list.into_any().into_py(py)
            }))
        })
    }

    fn __repr__(&self) -> String {
        "AsyncStream()".to_string()
    }
}
