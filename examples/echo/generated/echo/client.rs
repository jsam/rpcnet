#![allow(dead_code)]
#![allow(unused_imports)]
use super::types::*;
use rpcnet::{RpcClient, RpcConfig, RpcError};
use std::net::SocketAddr;
/// Generated client for calling service methods.
pub struct EchoClient {
    inner: RpcClient,
}
impl EchoClient {
    /// Connects to the service at the given address.
    pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        let inner = RpcClient::connect(addr, config).await?;
        Ok(Self { inner })
    }
    pub async fn echo(&self, request: EchoRequest) -> Result<EchoResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("Echo.echo", params).await?;
        bincode::deserialize::<EchoResponse>(&response_data).map_err(RpcError::SerializationError)
    }
    pub async fn binary_echo(
        &self,
        request: BinaryEchoRequest,
    ) -> Result<BinaryEchoResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("Echo.binary_echo", params).await?;
        bincode::deserialize::<BinaryEchoResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
}
