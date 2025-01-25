use serde::{Deserialize, Serialize};

use crate::errors::RpcError;


// RPC Response structure
#[derive(Serialize, Deserialize)]
pub struct RpcResponse {
    id: u64,
    result: Option<Vec<u8>>,
    error: Option<String>,
}

impl RpcResponse {
    pub fn new(id: u64, result: Option<Vec<u8>>, error: Option<String>) -> Self {
        Self { id, result, error }
    }

    pub fn from_result(id: u64, result: Result<Vec<u8>, RpcError>) -> Self {
        let (result, error) = match result {
            Ok(vec) => (Some(vec), None),
            Err(e) => (None, Some(e.to_string()))
        };

        Self { id, result, error }
    }
    
    pub fn id_mut(&mut self) -> &mut u64 {
        &mut self.id
    }
    
    pub fn id(&self) -> u64 {
        self.id
    }
    
    pub fn result(&self) -> Option<&Vec<u8>> {
        self.result.as_ref()
    }
    
    pub fn error(&self) -> Option<&String> {
        self.error.as_ref()
    }
    
}