use super::types::*;
use rpcnet::{RpcClient, RpcConfig, RpcError};
use std::net::SocketAddr;
/// Generated client for calling service methods.
pub struct GreetingClient {
    inner: RpcClient,
}
impl GreetingClient {
    /// Connects to the service at the given address.
    pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        let inner = RpcClient::connect(addr, config).await?;
        Ok(Self { inner })
    }
    pub async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("Greeting.greet", params).await?;
        bincode::deserialize::<GreetResponse>(&response_data).map_err(RpcError::SerializationError)
    }
}
