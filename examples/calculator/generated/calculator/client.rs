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
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("Calculator.add", params).await?;
        bincode::deserialize::<AddResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
    pub async fn subtract(
        &self,
        request: SubtractRequest,
    ) -> Result<SubtractResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("Calculator.subtract", params).await?;
        bincode::deserialize::<SubtractResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
    pub async fn multiply(
        &self,
        request: MultiplyRequest,
    ) -> Result<MultiplyResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("Calculator.multiply", params).await?;
        bincode::deserialize::<MultiplyResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
    pub async fn divide(
        &self,
        request: DivideRequest,
    ) -> Result<DivideResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("Calculator.divide", params).await?;
        bincode::deserialize::<DivideResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
}
