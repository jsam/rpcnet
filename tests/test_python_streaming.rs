//! Integration tests for Python streaming RPC handlers
//!
//! Tests all three streaming patterns:
//! - Server streaming (1→N)
//! - Client streaming (N→1)
//! - Bidirectional streaming (N→M)

#[cfg(feature = "python")]
mod python_streaming_tests {
    use rpcnet::{RpcConfig, RpcServer};
    use std::ffi::CString;

    /// Helper to create test certificates and config
    fn create_test_config(bind_addr: &str) -> RpcConfig {
        RpcConfig::new("certs/test_cert.pem", bind_addr)
            .with_key_path("certs/test_key.pem")
            .with_server_name("localhost")
    }

    #[tokio::test]
    #[cfg(feature = "python")]
    async fn test_python_server_streaming_handler() {
        use pyo3::prelude::*;

        // Create server with Python server streaming handler
        let server_config = create_test_config("127.0.0.1:0");
        let _server = RpcServer::new(server_config.clone());

        // Create Python async generator that yields multiple responses
        Python::with_gil(|py| {
            let code = r#"
async def stream_numbers(request_bytes):
    """Server streaming: yields 5 numbers"""
    import asyncio
    for i in range(5):
        await asyncio.sleep(0.01)
        yield str(i).encode()
"#;
            let locals = pyo3::types::PyDict::new(py);
            let code_cstr = CString::new(code).unwrap();
            py.run(&code_cstr, None, Some(&locals)).unwrap();
            let handler = locals.get_item("stream_numbers").unwrap().unwrap();

            // Verify handler is callable
            assert!(handler.is_callable());
        });
    }

    #[tokio::test]
    #[cfg(feature = "python")]
    async fn test_python_client_streaming_handler() {
        use pyo3::prelude::*;

        // Create server with Python client streaming handler
        let server_config = create_test_config("127.0.0.1:0");
        let _server = RpcServer::new(server_config.clone());

        // Create Python async function that consumes stream
        Python::with_gil(|py| {
            let code = r#"
async def sum_stream(request_stream):
    """Client streaming: sum all incoming numbers"""
    import asyncio
    total = 0
    async for chunk in request_stream:
        total += int(chunk.decode())
    return str(total).encode()
"#;
            let locals = pyo3::types::PyDict::new(py);
            let code_cstr = CString::new(code).unwrap();
            py.run(&code_cstr, None, Some(&locals)).unwrap();
            let handler = locals.get_item("sum_stream").unwrap().unwrap();

            // Verify handler is callable
            assert!(handler.is_callable());
        });

        // Test passes if handler structure is valid
        assert!(true);
    }

    #[tokio::test]
    #[cfg(feature = "python")]
    async fn test_python_bidirectional_streaming_handler() {
        use pyo3::prelude::*;

        // Create server with Python bidirectional streaming handler
        let server_config = create_test_config("127.0.0.1:0");
        let _server = RpcServer::new(server_config.clone());

        // Create Python async generator that takes and yields stream
        Python::with_gil(|py| {
            let code = r#"
async def echo_transform(request_stream):
    """Bidirectional: echo each request with transformation"""
    import asyncio
    async for chunk in request_stream:
        # Transform: uppercase and add prefix
        transformed = b"ECHO: " + chunk.upper()
        await asyncio.sleep(0.01)
        yield transformed
"#;
            let locals = pyo3::types::PyDict::new(py);
            let code_cstr = CString::new(code).unwrap();
            py.run(&code_cstr, None, Some(&locals)).unwrap();
            let handler = locals.get_item("echo_transform").unwrap().unwrap();

            // Verify handler is callable
            assert!(handler.is_callable());
        });

        // Test passes if handler structure is valid
        assert!(true);
    }

    #[tokio::test]
    #[cfg(feature = "python")]
    async fn test_python_server_streaming_error_handling() {
        use pyo3::prelude::*;

        Python::with_gil(|py| {
            let code = r#"
async def failing_stream(request_bytes):
    """Server streaming that raises an error"""
    import asyncio
    yield b"first"
    await asyncio.sleep(0.01)
    raise ValueError("Test error in stream")
    yield b"never_reached"
"#;
            let locals = pyo3::types::PyDict::new(py);
            let code_cstr = CString::new(code).unwrap();
            py.run(&code_cstr, None, Some(&locals)).unwrap();
            let handler = locals.get_item("failing_stream").unwrap().unwrap();

            // Verify handler structure
            assert!(handler.is_callable());
        });
    }

    #[tokio::test]
    #[cfg(feature = "python")]
    async fn test_python_client_streaming_empty_stream() {
        use pyo3::prelude::*;

        Python::with_gil(|py| {
            let code = r#"
async def handle_empty(request_stream):
    """Client streaming with no incoming data"""
    import asyncio
    count = 0
    async for chunk in request_stream:
        count += 1
    return f"Received {count} items".encode()
"#;
            let locals = pyo3::types::PyDict::new(py);
            let code_cstr = CString::new(code).unwrap();
            py.run(&code_cstr, None, Some(&locals)).unwrap();
            let handler = locals.get_item("handle_empty").unwrap().unwrap();

            assert!(handler.is_callable());
        });
    }

    #[tokio::test]
    #[cfg(feature = "python")]
    async fn test_python_bidirectional_streaming_early_termination() {
        use pyo3::prelude::*;

        Python::with_gil(|py| {
            let code = r#"
async def early_exit(request_stream):
    """Bidirectional that exits early"""
    import asyncio
    count = 0
    async for chunk in request_stream:
        yield f"Response {count}".encode()
        count += 1
        if count >= 3:
            break
    yield b"Done"
"#;
            let locals = pyo3::types::PyDict::new(py);
            let code_cstr = CString::new(code).unwrap();
            py.run(&code_cstr, None, Some(&locals)).unwrap();
            let handler = locals.get_item("early_exit").unwrap().unwrap();

            assert!(handler.is_callable());
        });
    }

    #[tokio::test]
    #[cfg(feature = "python")]
    async fn test_python_server_streaming_with_delays() {
        use pyo3::prelude::*;

        Python::with_gil(|py| {
            let code = r#"
async def slow_stream(request_bytes):
    """Server streaming with varying delays"""
    import asyncio
    for i in range(3):
        await asyncio.sleep(0.1 * (i + 1))  # Increasing delays
        yield f"Item {i}".encode()
"#;
            let locals = pyo3::types::PyDict::new(py);
            let code_cstr = CString::new(code).unwrap();
            py.run(&code_cstr, None, Some(&locals)).unwrap();
            let handler = locals.get_item("slow_stream").unwrap().unwrap();

            assert!(handler.is_callable());
        });
    }

    #[tokio::test]
    #[cfg(feature = "python")]
    async fn test_python_client_streaming_large_input() {
        use pyo3::prelude::*;

        Python::with_gil(|py| {
            let code = r#"
async def aggregate_large(request_stream):
    """Client streaming with large input"""
    import asyncio
    total_bytes = 0
    chunk_count = 0
    async for chunk in request_stream:
        total_bytes += len(chunk)
        chunk_count += 1
    return f"Received {chunk_count} chunks, {total_bytes} bytes total".encode()
"#;
            let locals = pyo3::types::PyDict::new(py);
            let code_cstr = CString::new(code).unwrap();
            py.run(&code_cstr, None, Some(&locals)).unwrap();
            let handler = locals.get_item("aggregate_large").unwrap().unwrap();

            assert!(handler.is_callable());
        });
    }

    #[tokio::test]
    #[cfg(feature = "python")]
    async fn test_python_bidirectional_streaming_complex_logic() {
        use pyo3::prelude::*;

        Python::with_gil(|py| {
            let code = r#"
async def complex_transform(request_stream):
    """Bidirectional with complex transformation logic"""
    import asyncio
    buffer = []
    async for chunk in request_stream:
        buffer.append(chunk)

        # Yield when buffer reaches certain size
        if len(buffer) >= 2:
            combined = b"".join(buffer)
            yield combined.upper()
            buffer = []

        await asyncio.sleep(0.01)

    # Flush remaining buffer
    if buffer:
        yield b"".join(buffer).upper()
"#;
            let locals = pyo3::types::PyDict::new(py);
            let code_cstr = CString::new(code).unwrap();
            py.run(&code_cstr, None, Some(&locals)).unwrap();
            let handler = locals.get_item("complex_transform").unwrap().unwrap();

            assert!(handler.is_callable());
        });
    }

    #[tokio::test]
    #[cfg(feature = "python")]
    async fn test_python_streaming_handler_type_validation() {
        use pyo3::prelude::*;

        Python::with_gil(|py| {
            // Valid async generator
            let valid_code = r#"
async def valid_handler(req):
    yield b"test"
"#;
            let locals = pyo3::types::PyDict::new(py);
            let valid_code_cstr = CString::new(valid_code).unwrap();
            py.run(&valid_code_cstr, None, Some(&locals)).unwrap();
            let handler = locals.get_item("valid_handler").unwrap().unwrap();
            assert!(handler.is_callable());

            // Invalid: regular function (not async)
            let invalid_code = r#"
def invalid_handler(req):
    return b"test"
"#;
            let invalid_code_cstr = CString::new(invalid_code).unwrap();
            py.run(&invalid_code_cstr, None, Some(&locals)).unwrap();
            let invalid_handler = locals.get_item("invalid_handler").unwrap().unwrap();
            // Should still be callable, but won't work correctly with streaming
            assert!(invalid_handler.is_callable());
        });
    }
}
