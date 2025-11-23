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
    pub async fn infer(
        &self,
        request: InferenceRequest,
    ) -> Result<InferenceResponse, RpcError> {
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self.inner.call("Inference.infer", params).await?;
        rmp_serde::from_slice::<InferenceResponse>(&response_data).map_err(Into::into)
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
            .map(|item| { rmp_serde::to_vec(&item).unwrap() });
        let byte_response_stream = self
            .inner
            .call_streaming("Inference.generate", Box::pin(byte_request_stream))
            .await?;
        let typed_response_stream = byte_response_stream
            .map(|result| {
                match result {
                    Ok(bytes) => {
                        rmp_serde::from_slice::<
                            Result<InferenceResponse, InferenceError>,
                        >(&bytes)
                            .expect("Failed to deserialize stream item")
                    }
                    Err(e) => {
                        panic!(
                            "Stream transport error: {:?}. Consider handling this at the caller level.",
                            e
                        )
                    }
                }
            });
        Ok(Box::pin(typed_response_stream))
    }
}
