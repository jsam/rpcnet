use super::types::*;
use rpcnet::{RpcServer, RpcConfig, RpcError};
use async_trait::async_trait;
use std::sync::Arc;
use futures::Stream;
use std::pin::Pin;
/// Handler trait that users implement for the service.
#[async_trait]
pub trait InferenceHandler: Send + Sync + 'static {
    async fn generate(
        &self,
        request: Pin<Box<dyn Stream<Item = InferenceRequest> + Send>>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<InferenceResponse, InferenceError>> + Send>>,
        InferenceError,
    >;
}
/// Generated server that manages RPC registration and routing.
pub struct InferenceServer<H: InferenceHandler> {
    handler: Arc<H>,
    pub rpc_server: RpcServer,
}
impl<H: InferenceHandler> InferenceServer<H> {
    /// Creates a new server with the given handler and configuration.
    pub fn new(handler: H, config: RpcConfig) -> Self {
        Self {
            handler: Arc::new(handler),
            rpc_server: RpcServer::new(config),
        }
    }
    /// Registers all service methods with the RPC server.
    pub async fn register_all(&mut self) {
        {
            let handler = self.handler.clone();
            self.rpc_server
                .register_streaming(
                    "Inference.generate",
                    move |request_stream| {
                        let handler = handler.clone();
                        async move {
                            use futures::StreamExt;
                            let typed_request_stream = request_stream
                                .map(|bytes| {
                                    bincode::deserialize::<InferenceRequest>(&bytes).unwrap()
                                });
                            match handler.generate(Box::pin(typed_request_stream)).await
                            {
                                Ok(response_stream) => {
                                    let byte_response_stream = response_stream
                                        .map(|item| { Ok(bincode::serialize(&item).unwrap()) });
                                    Box::pin(byte_response_stream)
                                        as Pin<
                                            Box<dyn Stream<Item = Result<Vec<u8>, RpcError>> + Send>,
                                        >
                                }
                                Err(e) => {
                                    Box::pin(
                                        futures::stream::once(async move {
                                            Err(RpcError::StreamError(format!("{:?}", e)))
                                        }),
                                    )
                                }
                            }
                        }
                    },
                )
                .await;
        }
    }
    /// Starts the server and begins accepting connections.
    pub async fn serve(mut self) -> Result<(), RpcError> {
        self.register_all().await;
        let quic_server = self.rpc_server.bind()?;
        println!("Server listening on: {:?}", self.rpc_server.socket_addr);
        self.rpc_server.start(quic_server).await
    }
}
