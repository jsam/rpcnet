//! Python wrappers for streaming RPC support
//!
//! This module provides Python bindings for streaming operations, allowing
//! Python code to consume Rust streams as async iterators.

#![allow(clippy::useless_conversion)]
#![allow(clippy::type_complexity)]

use super::error::to_py_err;
use futures::stream::{Stream, StreamExt};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Type alias for the inner stream type
type InnerStream = Arc<Mutex<Pin<Box<dyn Stream<Item = Result<Vec<u8>, crate::RpcError>> + Send>>>>;

/// Python wrapper for async stream (async iterator)
///
/// This allows Python code to consume Rust streams using async for:
/// ```python
/// async for data in stream:
///     print(data)
/// ```
#[pyclass(name = "AsyncStream")]
pub struct PyAsyncStream {
    inner: InnerStream,
}

impl PyAsyncStream {
    /// Create a new AsyncStream from a Rust stream
    pub fn new(
        stream: Pin<Box<dyn Stream<Item = Result<Vec<u8>, crate::RpcError>> + Send>>,
    ) -> Self {
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
                    Ok(Python::with_gil(|py| PyBytes::new(py, &data).unbind()))
                }
                Some(Err(e)) => {
                    // Convert error and raise in Python
                    Err(to_py_err(e))
                }
                None => {
                    // End of stream - raise StopAsyncIteration
                    Err(pyo3::exceptions::PyStopAsyncIteration::new_err(
                        "Stream ended",
                    ))
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
                let py_list = pyo3::types::PyList::empty(py);
                for item in items {
                    let _ = py_list.append(PyBytes::new(py, &item));
                }
                py_list.into_any().unbind()
            }))
        })
    }

    fn __repr__(&self) -> String {
        "AsyncStream()".to_string()
    }
}

#[cfg(all(test, feature = "python"))]
mod tests {
    use super::*;
    use crate::RpcError;
    use futures::stream;

    #[test]
    fn test_repr() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|_py| {
            let stream = stream::iter(vec![Ok(vec![1, 2, 3])]);
            let py_stream = PyAsyncStream::new(Box::pin(stream));

            assert_eq!(py_stream.__repr__(), "AsyncStream()");
        });
    }

    #[test]
    fn test_new_creates_stream() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|_py| {
            let stream = stream::iter(vec![Ok(vec![1, 2, 3]), Ok(vec![4, 5, 6])]);
            let py_stream = PyAsyncStream::new(Box::pin(stream));

            // Just verify it was created successfully
            assert_eq!(py_stream.__repr__(), "AsyncStream()");
        });
    }

    #[tokio::test]
    async fn test_stream_with_single_item() {
        pyo3::prepare_freethreaded_python();

        let stream = stream::iter(vec![Ok(vec![42u8])]);
        let py_stream = PyAsyncStream::new(Box::pin(stream));

        // Manually pull one item
        let mut stream_guard = py_stream.inner.lock().await;
        let item = stream_guard.next().await;

        assert!(item.is_some());
        let result = item.unwrap();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![42u8]);
    }

    #[tokio::test]
    async fn test_stream_with_multiple_items() {
        pyo3::prepare_freethreaded_python();

        let stream = stream::iter(vec![
            Ok(vec![1u8, 2u8]),
            Ok(vec![3u8, 4u8]),
            Ok(vec![5u8, 6u8]),
        ]);
        let py_stream = PyAsyncStream::new(Box::pin(stream));

        let mut stream_guard = py_stream.inner.lock().await;

        // First item
        let item1 = stream_guard.next().await.unwrap().unwrap();
        assert_eq!(item1, vec![1u8, 2u8]);

        // Second item
        let item2 = stream_guard.next().await.unwrap().unwrap();
        assert_eq!(item2, vec![3u8, 4u8]);

        // Third item
        let item3 = stream_guard.next().await.unwrap().unwrap();
        assert_eq!(item3, vec![5u8, 6u8]);

        // Stream should be exhausted
        let item4 = stream_guard.next().await;
        assert!(item4.is_none());
    }

    #[tokio::test]
    async fn test_stream_with_error() {
        pyo3::prepare_freethreaded_python();

        let stream = stream::iter(vec![
            Ok(vec![1u8, 2u8]),
            Err(RpcError::StreamError("test error".to_string())),
            Ok(vec![3u8, 4u8]),
        ]);
        let py_stream = PyAsyncStream::new(Box::pin(stream));

        let mut stream_guard = py_stream.inner.lock().await;

        // First item should succeed
        let item1 = stream_guard.next().await.unwrap();
        assert!(item1.is_ok());

        // Second item should be an error
        let item2 = stream_guard.next().await.unwrap();
        assert!(item2.is_err());
        let err = item2.unwrap_err();
        assert!(matches!(err, RpcError::StreamError(_)));

        // Third item should still be accessible
        let item3 = stream_guard.next().await.unwrap();
        assert!(item3.is_ok());
    }

    #[tokio::test]
    async fn test_empty_stream() {
        pyo3::prepare_freethreaded_python();

        let stream: Pin<Box<dyn Stream<Item = Result<Vec<u8>, RpcError>> + Send>> =
            Box::pin(stream::iter(vec![]));
        let py_stream = PyAsyncStream::new(stream);

        let mut stream_guard = py_stream.inner.lock().await;

        // Should be immediately exhausted
        let item = stream_guard.next().await;
        assert!(item.is_none());
    }

    #[tokio::test]
    async fn test_stream_with_large_data() {
        pyo3::prepare_freethreaded_python();

        let large_data = vec![42u8; 10_000];
        let stream = stream::iter(vec![Ok(large_data.clone())]);
        let py_stream = PyAsyncStream::new(Box::pin(stream));

        let mut stream_guard = py_stream.inner.lock().await;
        let item = stream_guard.next().await.unwrap().unwrap();

        assert_eq!(item.len(), 10_000);
        assert_eq!(item, large_data);
    }

    #[tokio::test]
    async fn test_stream_mutex_isolation() {
        pyo3::prepare_freethreaded_python();

        let stream = stream::iter(vec![Ok(vec![1u8]), Ok(vec![2u8])]);
        let py_stream = PyAsyncStream::new(Box::pin(stream));

        // Lock the stream
        let mut guard1 = py_stream.inner.lock().await;

        // Try to lock again (should wait, but we'll just verify the first lock works)
        let item = guard1.next().await.unwrap().unwrap();
        assert_eq!(item, vec![1u8]);

        drop(guard1); // Release lock

        // Now we can lock again
        let mut guard2 = py_stream.inner.lock().await;
        let item = guard2.next().await.unwrap().unwrap();
        assert_eq!(item, vec![2u8]);
    }
}
