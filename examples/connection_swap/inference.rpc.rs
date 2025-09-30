use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub prompt: String,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub token: String,
    pub connection_id: String,
    pub worker_id: String,
    pub sequence: u32,
}

service Inference {
    rpc Generate(stream GenerateRequest) -> (stream GenerateResponse);
}