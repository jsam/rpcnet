#![allow(dead_code)]
#![allow(unused_imports)]
use super::types::*;
use rpcnet::{RpcClient, RpcConfig, RpcError};
use std::net::SocketAddr;
use futures::Stream;
use std::pin::Pin;
/// Generated client for calling service methods.
pub struct InferenceClient {
    inner: RpcClient,
}
impl InferenceClient {
    /// Connects to the service at the given address.
    pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        let inner = RpcClient::connect(addr, config).await?;
        Ok(Self { inner })
    }
    pub async fn generate(
        &self,
        request: Pin<Box<dyn Stream<Item = InferenceRequest> + Send>>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<InferenceResponse, InferenceError>> + Send>>,
        RpcError,
    > {
        use futures::StreamExt;
        let byte_request_stream = request
            .map(|item| { bincode::serialize(&item).unwrap() });
        let byte_response_stream = self
            .inner
            .call_streaming("Inference.generate", Box::pin(byte_request_stream))
            .await?;
        let typed_response_stream = byte_response_stream
            .map(|result| {
                match result {
                    Ok(bytes) => {
                        bincode::deserialize::<
                            Result<InferenceResponse, InferenceError>,
                        >(&bytes)
                            .map_err(|_| InferenceError::InvalidRequest("Deserialization failed".to_string()))
                            .and_then(|inner_result| inner_result)
                    }
                    Err(rpcnet::streaming::StreamError::Timeout) => {
                        Err(InferenceError::WorkerFailed("Timeout waiting for response".to_string()))
                    }
                    Err(rpcnet::streaming::StreamError::Transport(e)) => {
                        Err(InferenceError::WorkerFailed(format!("Network error: {}", e)))
                    }
                    Err(rpcnet::streaming::StreamError::Item(_)) => {
                        Err(InferenceError::InvalidRequest("Unexpected item error".to_string()))
                    }
                }
            });
        Ok(Box::pin(typed_response_stream))
    }
}
