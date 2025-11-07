use super::types::*;
use rpcnet::{RpcClient, RpcConfig, RpcError};
use std::net::SocketAddr;
/// Generated client for calling service methods.
pub struct CalculatorClient {
    inner: RpcClient,
}
impl CalculatorClient {
    /// Connects to the service at the given address.
    pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        let inner = RpcClient::connect(addr, config).await?;
        Ok(Self { inner })
    }
    pub async fn add(&self, request: AddRequest) -> Result<AddResponse, RpcError> {
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self.inner.call("Calculator.add", params).await?;
        rmp_serde::from_slice::<AddResponse>(&response_data).map_err(Into::into)
    }
    pub async fn subtract(&self, request: SubtractRequest) -> Result<SubtractResponse, RpcError> {
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self.inner.call("Calculator.subtract", params).await?;
        rmp_serde::from_slice::<SubtractResponse>(&response_data).map_err(Into::into)
    }
    pub async fn multiply(&self, request: MultiplyRequest) -> Result<MultiplyResponse, RpcError> {
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self.inner.call("Calculator.multiply", params).await?;
        rmp_serde::from_slice::<MultiplyResponse>(&response_data).map_err(Into::into)
    }
    pub async fn divide(&self, request: DivideRequest) -> Result<DivideResponse, RpcError> {
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self.inner.call("Calculator.divide", params).await?;
        rmp_serde::from_slice::<DivideResponse>(&response_data).map_err(Into::into)
    }
}
