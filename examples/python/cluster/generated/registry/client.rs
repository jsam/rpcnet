use super::types::*;
use rpcnet::{RpcClient, RpcConfig, RpcError};
use std::net::SocketAddr;
/// Generated client for calling service methods.
pub struct RegistryClient {
    inner: RpcClient,
}
impl RegistryClient {
    /// Connects to the service at the given address.
    pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        let inner = RpcClient::connect(addr, config).await?;
        Ok(Self { inner })
    }
    pub async fn get_worker(
        &self,
        request: GetWorkerRequest,
    ) -> Result<GetWorkerResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("Registry.get_worker", params).await?;
        bincode::deserialize::<GetWorkerResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
}
