use super::types::*;
use rpcnet::{RpcClient, RpcConfig, RpcError};
use std::net::SocketAddr;
/// Generated client for calling service methods.
pub struct ConcurrentDemoClient {
    inner: RpcClient,
}
impl ConcurrentDemoClient {
    /// Connects to the service at the given address.
    pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        let inner = RpcClient::connect(addr, config).await?;
        Ok(Self { inner })
    }
    pub async fn compute(
        &self,
        request: ComputeRequest,
    ) -> Result<ComputeResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("ConcurrentDemo.compute", params).await?;
        bincode::deserialize::<ComputeResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
    pub async fn async_task(
        &self,
        request: AsyncTaskRequest,
    ) -> Result<AsyncTaskResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("ConcurrentDemo.async_task", params).await?;
        bincode::deserialize::<AsyncTaskResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
    pub async fn increment(
        &self,
        request: IncrementRequest,
    ) -> Result<IncrementResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("ConcurrentDemo.increment", params).await?;
        bincode::deserialize::<IncrementResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
    pub async fn get_counter(
        &self,
        request: GetCounterRequest,
    ) -> Result<GetCounterResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("ConcurrentDemo.get_counter", params).await?;
        bincode::deserialize::<GetCounterResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
}
