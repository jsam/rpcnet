use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingRequest {
    pub payload: Vec<u8>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResponse {
    pub payload: Vec<u8>,
    pub timestamp: u64,
    pub server_timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRequest {
    pub message: String,
    pub value: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResponse {
    pub echo: String,
    pub doubled: i32,
    pub server_time_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoopRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoopResponse {
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkError {
    pub message: String,
}

#[rpcnet::service]
pub trait BenchmarkService {
    /// Simple ping with payload echo
    async fn ping(&self, request: PingRequest) -> Result<PingResponse, BenchmarkError>;
    
    /// Echo and double benchmark
    async fn process(&self, request: BenchmarkRequest) -> Result<BenchmarkResponse, BenchmarkError>;
    
    /// No-op for measuring pure overhead
    async fn noop(&self, request: NoopRequest) -> Result<NoopResponse, BenchmarkError>;
}