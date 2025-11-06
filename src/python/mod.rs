//! Python bindings for RpcNet
//!
//! This module provides Python bindings via PyO3, exposing RpcNet's functionality
//! to Python with async/await support through asyncio.

#[cfg(feature = "python")]
pub mod client;
#[cfg(feature = "python")]
pub mod config;
#[cfg(feature = "python")]
pub mod error;
#[cfg(feature = "python")]
pub mod serde;
#[cfg(feature = "python")]
pub mod server;
#[cfg(feature = "python")]
pub mod streaming;

#[cfg(feature = "python")]
use pyo3::prelude::*;

/// Main Python module initialization
/// This creates the `_rpcnet` module that Python code imports
#[cfg(feature = "python")]
#[pymodule]
fn _rpcnet(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register classes
    m.add_class::<config::PyRpcConfig>()?;
    m.add_class::<client::PyRpcClient>()?;
    m.add_class::<server::PyRpcServer>()?;
    m.add_class::<streaming::PyAsyncStream>()?;

    // Register exception types
    m.add("RpcError", py.get_type::<error::PyRpcError>())?;
    m.add(
        "ConnectionError",
        py.get_type::<error::PyConnectionError>(),
    )?;
    m.add("TimeoutError", py.get_type::<error::PyTimeoutError>())?;
    m.add(
        "SerializationError",
        py.get_type::<error::PySerializationError>(),
    )?;
    m.add("TlsError", py.get_type::<error::PyTlsError>())?;

    // Register serialization functions
    m.add_function(wrap_pyfunction!(serde::python_to_bincode_py, m)?)?;
    m.add_function(wrap_pyfunction!(serde::bincode_to_python_py, m)?)?;
    m.add_function(wrap_pyfunction!(serde::python_to_msgpack_py, m)?)?;
    m.add_function(wrap_pyfunction!(serde::msgpack_to_python_py, m)?)?;

    Ok(())
}
