//! Python wrapper for RpcClient

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use crate::RpcClient;
use super::{config::PyRpcConfig, error::to_py_err, streaming::PyAsyncStream};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use futures::stream::StreamExt;
use async_stream::stream;

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

    /// Call a server streaming RPC method (one request, multiple responses)
    ///
    /// Server streaming means the client sends one request and receives
    /// multiple response messages as a stream.
    ///
    /// Args:
    ///     method: Method name to call
    ///     params: Request data as bytes
    ///
    /// Returns:
    ///     AsyncStream: Async iterator over response messages
    ///
    /// Raises:
    ///     TimeoutError: If request times out
    ///     ConnectionError: If connection is lost
    ///     RpcError: For other RPC errors
    ///
    /// Example:
    ///     >>> stream = await client.call_server_streaming("list_items", request_bytes)
    ///     >>> async for item_bytes in stream:
    ///     ...     item = deserialize(item_bytes)
    ///     ...     print(item)
    fn call_server_streaming<'py>(
        &self,
        py: Python<'py>,
        method: String,
        params: Vec<u8>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.client.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let response_stream = client
                .call_server_streaming(&method, params)
                .await
                .map_err(to_py_err)?;

            // Map StreamError to RpcError
            let mapped_stream = response_stream.map(|result| {
                result.map_err(|stream_err| match stream_err {
                    crate::streaming::StreamError::Timeout => crate::RpcError::Timeout,
                    crate::streaming::StreamError::Transport(e) => e,
                    crate::streaming::StreamError::Item(e) => e,
                })
            });

            Ok(PyAsyncStream::new(Box::pin(mapped_stream)))
        })
    }

    /// Call a client streaming RPC method (multiple requests, one response)
    ///
    /// Client streaming means the client sends multiple request messages
    /// and receives a single response.
    ///
    /// Args:
    ///     method: Method name to call
    ///     request_list: List of request data as bytes
    ///
    /// Returns:
    ///     bytes: Single response data
    ///
    /// Raises:
    ///     TimeoutError: If request times out
    ///     ConnectionError: If connection is lost
    ///     RpcError: For other RPC errors
    ///
    /// Example:
    ///     >>> requests = [b"chunk1", b"chunk2", b"chunk3"]
    ///     >>> response = await client.call_client_streaming("upload", requests)
    fn call_client_streaming<'py>(
        &self,
        py: Python<'py>,
        method: String,
        request_list: Vec<Vec<u8>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.client.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Convert Vec to async stream (Stream<Item = Vec<u8>>)
            let request_stream = stream! {
                for data in request_list {
                    yield data;
                }
            };

            let response = client
                .call_client_streaming(&method, request_stream)
                .await
                .map_err(to_py_err)?;

            Ok(Python::with_gil(|py| PyBytes::new_bound(py, &response).into_py(py)))
        })
    }

    /// Call a bidirectional streaming RPC method (multiple requests, multiple responses)
    ///
    /// Bidirectional streaming means both client and server send multiple messages.
    ///
    /// Args:
    ///     method: Method name to call
    ///     request_list: List of request data as bytes
    ///
    /// Returns:
    ///     AsyncStream: Async iterator over response messages
    ///
    /// Raises:
    ///     TimeoutError: If request times out
    ///     ConnectionError: If connection is lost
    ///     RpcError: For other RPC errors
    ///
    /// Example:
    ///     >>> requests = [b"msg1", b"msg2", b"msg3"]
    ///     >>> stream = await client.call_streaming("chat", requests)
    ///     >>> async for response_bytes in stream:
    ///     ...     response = deserialize(response_bytes)
    ///     ...     print(response)
    fn call_streaming<'py>(
        &self,
        py: Python<'py>,
        method: String,
        request_list: Vec<Vec<u8>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let client = self.client.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Convert Vec to async stream (Stream<Item = Vec<u8>>)
            let request_stream = stream! {
                for data in request_list {
                    yield data;
                }
            };

            let response_stream = client
                .call_streaming(&method, request_stream)
                .await
                .map_err(to_py_err)?;

            // Map StreamError to RpcError
            let mapped_stream = response_stream.map(|result| {
                result.map_err(|stream_err| match stream_err {
                    crate::streaming::StreamError::Timeout => crate::RpcError::Timeout,
                    crate::streaming::StreamError::Transport(e) => e,
                    crate::streaming::StreamError::Item(e) => e,
                })
            });

            Ok(PyAsyncStream::new(Box::pin(mapped_stream)))
        })
    }

    fn __repr__(&self) -> String {
        "RpcClient(connected)".to_string()
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}
