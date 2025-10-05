use serde::{Deserialize, Serialize};
use futures::Stream;
use std::pin::Pin;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub connection_id: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InferenceResponse {
    Connected { worker: String, connection_id: String },
    Token { text: String, sequence: u64 },
    Error { message: String },
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InferenceError {
    WorkerFailed(String),
    InvalidRequest(String),
}

#[rpcnet::service]
pub trait Inference {
    async fn generate(
        &self,
        request: Pin<Box<dyn Stream<Item = InferenceRequest> + Send>>
    ) -> Result<Pin<Box<dyn Stream<Item = Result<InferenceResponse, InferenceError>> + Send>>, InferenceError>;
}
