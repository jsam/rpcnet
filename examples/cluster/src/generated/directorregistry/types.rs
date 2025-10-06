#![allow(dead_code)]
#![allow(unused_imports)]
//! Type definitions for the service.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWorkerResponse {
    pub success: bool,
    pub worker_addr: Option<String>,
    pub worker_label: Option<String>,
    pub connection_id: String,
    pub message: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DirectorError {
    NoWorkersAvailable,
    InvalidRequest(String),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWorkerRequest {
    pub connection_id: Option<String>,
    pub prompt: String,
}
