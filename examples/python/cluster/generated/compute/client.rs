use super::types::*;
use rpcnet::{RpcClient, RpcConfig, RpcError};
use std::net::SocketAddr;
/// Generated client for calling service methods.
pub struct ComputeClient {
    inner: RpcClient,
}
impl ComputeClient {
    /// Connects to the service at the given address.
    pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        let inner = RpcClient::connect(addr, config).await?;
        Ok(Self { inner })
    }
    pub async fn process(
        &self,
        request: ComputeRequest,
    ) -> Result<ComputeResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("Compute.process", params).await?;
        bincode::deserialize::<ComputeResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
}
