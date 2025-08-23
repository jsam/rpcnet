use super::types::*;
use rpcnet::{RpcServer, RpcConfig, RpcError};
use async_trait::async_trait;
use std::sync::Arc;
/// Handler trait that users implement for the service.
#[async_trait]
pub trait ConcurrentDemoHandler: Send + Sync + 'static {
    async fn compute(
        &self,
        request: ComputeRequest,
    ) -> Result<ComputeResponse, ConcurrentError>;
    async fn async_task(
        &self,
        request: AsyncTaskRequest,
    ) -> Result<AsyncTaskResponse, ConcurrentError>;
    async fn increment(
        &self,
        request: IncrementRequest,
    ) -> Result<IncrementResponse, ConcurrentError>;
    async fn get_counter(
        &self,
        request: GetCounterRequest,
    ) -> Result<GetCounterResponse, ConcurrentError>;
}
/// Generated server that manages RPC registration and routing.
pub struct ConcurrentDemoServer<H: ConcurrentDemoHandler> {
    handler: Arc<H>,
    rpc_server: RpcServer,
}
impl<H: ConcurrentDemoHandler> ConcurrentDemoServer<H> {
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
                .register(
                    "ConcurrentDemo.compute",
                    move |params| {
                        let handler = handler.clone();
                        async move {
                            let request: ComputeRequest = bincode::deserialize(&params)
                                .map_err(RpcError::SerializationError)?;
                            match handler.compute(request).await {
                                Ok(response) => {
                                    bincode::serialize(&response)
                                        .map_err(RpcError::SerializationError)
                                }
                                Err(e) => Err(RpcError::StreamError(format!("{:?}", e))),
                            }
                        }
                    },
                )
                .await;
        }
        {
            let handler = self.handler.clone();
            self.rpc_server
                .register(
                    "ConcurrentDemo.async_task",
                    move |params| {
                        let handler = handler.clone();
                        async move {
                            let request: AsyncTaskRequest = bincode::deserialize(&params)
                                .map_err(RpcError::SerializationError)?;
                            match handler.async_task(request).await {
                                Ok(response) => {
                                    bincode::serialize(&response)
                                        .map_err(RpcError::SerializationError)
                                }
                                Err(e) => Err(RpcError::StreamError(format!("{:?}", e))),
                            }
                        }
                    },
                )
                .await;
        }
        {
            let handler = self.handler.clone();
            self.rpc_server
                .register(
                    "ConcurrentDemo.increment",
                    move |params| {
                        let handler = handler.clone();
                        async move {
                            let request: IncrementRequest = bincode::deserialize(&params)
                                .map_err(RpcError::SerializationError)?;
                            match handler.increment(request).await {
                                Ok(response) => {
                                    bincode::serialize(&response)
                                        .map_err(RpcError::SerializationError)
                                }
                                Err(e) => Err(RpcError::StreamError(format!("{:?}", e))),
                            }
                        }
                    },
                )
                .await;
        }
        {
            let handler = self.handler.clone();
            self.rpc_server
                .register(
                    "ConcurrentDemo.get_counter",
                    move |params| {
                        let handler = handler.clone();
                        async move {
                            let request: GetCounterRequest = bincode::deserialize(
                                    &params,
                                )
                                .map_err(RpcError::SerializationError)?;
                            match handler.get_counter(request).await {
                                Ok(response) => {
                                    bincode::serialize(&response)
                                        .map_err(RpcError::SerializationError)
                                }
                                Err(e) => Err(RpcError::StreamError(format!("{:?}", e))),
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
