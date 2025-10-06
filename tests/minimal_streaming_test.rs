#![allow(clippy::all)]
#![allow(warnings)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::assertions_on_constants)]
use futures::StreamExt;
use rpcnet::{RpcClient, RpcConfig, RpcServer};
use std::time::Duration;

/// Helper function to create test configuration
fn test_config() -> RpcConfig {
    RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
}

#[tokio::test]
async fn test_simple_streaming() {
    // Start server with simple streaming handler
    let mut server = RpcServer::new(test_config());

    // Register a simple server streaming handler
    server
        .register_streaming("simple_stream", |_request_stream| async move {
            Box::pin(async_stream::stream! {
                eprintln!("[Handler] Starting simple stream handler");
                // Just send 3 fixed responses
                for i in 0..3 {
                    let msg = format!("response_{}", i);
                    eprintln!("[Handler] Sending: {}", msg);
                    yield Ok(msg.into_bytes());
                }
                eprintln!("[Handler] Handler finished");
            })
                as std::pin::Pin<
                    Box<dyn futures::Stream<Item = Result<Vec<u8>, rpcnet::RpcError>> + Send>,
                >
        })
        .await;

    let quic_server = server.bind().expect("Server should bind");
    let addr = quic_server.local_addr().expect("Should get address");

    let mut server_clone = server.clone();
    tokio::spawn(async move {
        server_clone
            .start(quic_server)
            .await
            .expect("Server should start");
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Connect client
    let client = RpcClient::connect(addr, test_config())
        .await
        .expect("Client should connect");

    // Use server streaming instead (single request, multiple responses)
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let response_stream = client
            .call_server_streaming("simple_stream", b"start".to_vec())
            .await?;
        let responses: Vec<_> = Box::pin(response_stream).collect().await;
        Ok::<_, rpcnet::RpcError>(responses)
    })
    .await;

    match result {
        Ok(Ok(responses)) => {
            eprintln!("Got {} responses", responses.len());
            for (i, response) in responses.iter().enumerate() {
                match response {
                    Ok(data) => {
                        let expected = format!("response_{}", i);
                        println!("Response {}: {}", i, String::from_utf8_lossy(data));
                        assert_eq!(data, &expected.as_bytes(), "Response should match expected");
                    }
                    Err(e) => panic!("Response error: {:?}", e),
                }
            }
        }
        Ok(Err(e)) => panic!("Server streaming failed: {:?}", e),
        Err(_) => panic!("Server streaming timed out"),
    }
}
