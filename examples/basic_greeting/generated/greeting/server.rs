use super::types::*;
use async_trait::async_trait;
use rpcnet::{RpcConfig, RpcError, RpcServer};
use std::sync::Arc;
/// Handler trait that users implement for the service.
#[async_trait]
pub trait GreetingHandler: Send + Sync + 'static {
    async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, GreetingError>;
}
/// Generated server that manages RPC registration and routing.
pub struct GreetingServer<H: GreetingHandler> {
    handler: Arc<H>,
    rpc_server: RpcServer,
}
impl<H: GreetingHandler> GreetingServer<H> {
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
                .register("Greeting.greet", move |params| {
                    let handler = handler.clone();
                    async move {
                        let request: GreetRequest =
                            bincode::deserialize(&params).map_err(RpcError::SerializationError)?;
                        match handler.greet(request).await {
                            Ok(response) => {
                                bincode::serialize(&response).map_err(RpcError::SerializationError)
                            }
                            Err(e) => Err(RpcError::StreamError(format!("{:?}", e))),
                        }
                    }
                })
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
