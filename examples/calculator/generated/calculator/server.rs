#![allow(dead_code)]
#![allow(unused_imports)]

use super::types::*;
use async_trait::async_trait;
use rpcnet::{RpcConfig, RpcError, RpcServer};
use std::sync::Arc;
/// Handler trait that users implement for the service.
#[async_trait]
pub trait CalculatorHandler: Send + Sync + 'static {
    async fn add(&self, request: AddRequest) -> Result<AddResponse, CalculatorError>;
    async fn subtract(&self, request: SubtractRequest)
        -> Result<SubtractResponse, CalculatorError>;
    async fn multiply(&self, request: MultiplyRequest)
        -> Result<MultiplyResponse, CalculatorError>;
    async fn divide(&self, request: DivideRequest) -> Result<DivideResponse, CalculatorError>;
}
/// Generated server that manages RPC registration and routing.
pub struct CalculatorServer<H: CalculatorHandler> {
    handler: Arc<H>,
    rpc_server: RpcServer,
}
impl<H: CalculatorHandler> CalculatorServer<H> {
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
                .register("Calculator.add", move |params| {
                    let handler = handler.clone();
                    async move {
                        let request: AddRequest =
                            bincode::deserialize(&params).map_err(RpcError::SerializationError)?;
                        match handler.add(request).await {
                            Ok(response) => {
                                bincode::serialize(&response).map_err(RpcError::SerializationError)
                            }
                            Err(e) => Err(RpcError::StreamError(format!("{:?}", e))),
                        }
                    }
                })
                .await;
        }
        {
            let handler = self.handler.clone();
            self.rpc_server
                .register("Calculator.subtract", move |params| {
                    let handler = handler.clone();
                    async move {
                        let request: SubtractRequest =
                            bincode::deserialize(&params).map_err(RpcError::SerializationError)?;
                        match handler.subtract(request).await {
                            Ok(response) => {
                                bincode::serialize(&response).map_err(RpcError::SerializationError)
                            }
                            Err(e) => Err(RpcError::StreamError(format!("{:?}", e))),
                        }
                    }
                })
                .await;
        }
        {
            let handler = self.handler.clone();
            self.rpc_server
                .register("Calculator.multiply", move |params| {
                    let handler = handler.clone();
                    async move {
                        let request: MultiplyRequest =
                            bincode::deserialize(&params).map_err(RpcError::SerializationError)?;
                        match handler.multiply(request).await {
                            Ok(response) => {
                                bincode::serialize(&response).map_err(RpcError::SerializationError)
                            }
                            Err(e) => Err(RpcError::StreamError(format!("{:?}", e))),
                        }
                    }
                })
                .await;
        }
        {
            let handler = self.handler.clone();
            self.rpc_server
                .register("Calculator.divide", move |params| {
                    let handler = handler.clone();
                    async move {
                        let request: DivideRequest =
                            bincode::deserialize(&params).map_err(RpcError::SerializationError)?;
                        match handler.divide(request).await {
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
