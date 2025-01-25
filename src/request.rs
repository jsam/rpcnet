use serde::{Deserialize, Serialize};


// RPC Request structure
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RpcRequest {
    id: u64,
    method: String,
    params: Vec<u8>,
}

impl RpcRequest {
    pub fn new(id: u64, method: String, params: Vec<u8>) -> Self {
        Self { id, method, params }
    }
    
    pub fn id(&self) -> u64 {
        self.id
    }
    
    pub fn method(&self) -> &str {
        &self.method
    }
    
    pub fn params(&self) -> &[u8] {
        &self.params
    }
}