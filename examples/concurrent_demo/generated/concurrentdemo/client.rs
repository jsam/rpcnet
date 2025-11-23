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
    pub async fn compute(&self, request: ComputeRequest) -> Result<ComputeResponse, RpcError> {
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self.inner.call("ConcurrentDemo.compute", params).await?;
        rmp_serde::from_slice::<ComputeResponse>(&response_data).map_err(Into::into)
    }
    pub async fn async_task(
        &self,
        request: AsyncTaskRequest,
    ) -> Result<AsyncTaskResponse, RpcError> {
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self.inner.call("ConcurrentDemo.async_task", params).await?;
        rmp_serde::from_slice::<AsyncTaskResponse>(&response_data).map_err(Into::into)
    }
    pub async fn increment(
        &self,
        request: IncrementRequest,
    ) -> Result<IncrementResponse, RpcError> {
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self.inner.call("ConcurrentDemo.increment", params).await?;
        rmp_serde::from_slice::<IncrementResponse>(&response_data).map_err(Into::into)
    }
    pub async fn get_counter(
        &self,
        request: GetCounterRequest,
    ) -> Result<GetCounterResponse, RpcError> {
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self
            .inner
            .call("ConcurrentDemo.get_counter", params)
            .await?;
        rmp_serde::from_slice::<GetCounterResponse>(&response_data).map_err(Into::into)
    }
}
