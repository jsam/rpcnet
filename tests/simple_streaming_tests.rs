#![allow(clippy::all)]
#![allow(warnings)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::type_complexity)]
#![allow(clippy::never_loop)]
#![allow(clippy::collapsible_if)]

use rpcnet::{RpcConfig, RpcServer};
use std::time::Duration;

/// Helper function to create test configuration
fn test_config() -> RpcConfig {
    RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
}

#[tokio::test]
async fn test_streaming_handler_registration() {
    let server = RpcServer::new(test_config());

    // Test that streaming handler registration doesn't crash
    server
        .register_streaming("test_stream", |_request_stream| async move {
            Box::pin(async_stream::stream! {
                yield Ok(b"test".to_vec());
            })
                as std::pin::Pin<
                    Box<dyn futures::Stream<Item = Result<Vec<u8>, rpcnet::RpcError>> + Send>,
                >
        })
        .await;

    // If we get here, registration worked
    assert!(true);
}

#[tokio::test]
async fn test_streaming_server_setup() {
    let mut server = RpcServer::new(test_config());

    // Register a simple streaming handler
    server
        .register_streaming("simple_stream", |_request_stream| async move {
            Box::pin(async_stream::stream! {
                for i in 0..3 {
                    yield Ok(format!("message_{}", i).into_bytes());
                }
            })
                as std::pin::Pin<
                    Box<dyn futures::Stream<Item = Result<Vec<u8>, rpcnet::RpcError>> + Send>,
                >
        })
        .await;

    // Test that server can bind with streaming handlers
    let result = server.bind();
    assert!(
        result.is_ok(),
        "Server should bind successfully with streaming handlers"
    );
}

#[tokio::test]
async fn test_streaming_types_compilation() {
    // This test just ensures the streaming types compile correctly
    fn create_handler() -> impl Fn(
        std::pin::Pin<Box<dyn futures::Stream<Item = Vec<u8>> + Send>>,
    ) -> std::pin::Pin<
        Box<
            dyn futures::Future<
                    Output = std::pin::Pin<
                        Box<dyn futures::Stream<Item = Result<Vec<u8>, rpcnet::RpcError>> + Send>,
                    >,
                > + Send,
        >,
    > + Send
           + Sync
           + Clone {
        |_request_stream| {
            Box::pin(async move {
                Box::pin(async_stream::stream! {
                    yield Ok(b"test".to_vec());
                })
                    as std::pin::Pin<
                        Box<dyn futures::Stream<Item = Result<Vec<u8>, rpcnet::RpcError>> + Send>,
                    >
            })
        }
    }

    let _handler = create_handler();
    assert!(true, "Streaming types should compile");
}

// Simple timeout test to ensure no infinite loops
#[tokio::test]
async fn test_streaming_no_infinite_loops() {
    let timeout_result = tokio::time::timeout(Duration::from_secs(5), async {
        let server = RpcServer::new(test_config());

        server
            .register_streaming("timeout_test", |_request_stream| async move {
                Box::pin(async_stream::stream! {
                    yield Ok(b"done".to_vec());
                })
                    as std::pin::Pin<
                        Box<dyn futures::Stream<Item = Result<Vec<u8>, rpcnet::RpcError>> + Send>,
                    >
            })
            .await;

        "completed"
    })
    .await;

    assert!(
        timeout_result.is_ok(),
        "Streaming registration should not hang"
    );
    assert_eq!(timeout_result.unwrap(), "completed");
}
