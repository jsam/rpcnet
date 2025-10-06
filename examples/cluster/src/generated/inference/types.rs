#![allow(dead_code)]
#![allow(unused_imports)]
//! Type definitions for the service.
use serde::{Deserialize, Serialize};
use futures::Stream;
use std::pin::Pin;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InferenceResponse {
    Connected { worker: String, connection_id: String },
    Token { text: String, sequence: u64 },
    Error { message: String },
    Done,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub connection_id: String,
    pub prompt: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InferenceError {
    WorkerFailed(String),
    InvalidRequest(String),
}
