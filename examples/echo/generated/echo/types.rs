//! Type definitions for the service.
use serde::{Deserialize, Serialize};
/// Response from echo operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EchoResponse {
    pub echoed_message: String,
}
/// Binary echo request for testing binary data.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BinaryEchoRequest {
    pub data: Vec<u8>,
}
/// Errors that can occur in echo operations.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum EchoError {
    /// Too many repetitions requested.
    TooManyRepetitions,
    /// Empty message provided.
    EmptyMessage,
    /// Data too large.
    DataTooLarge,
}
/// Request for echo operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EchoRequest {
    pub message: String,
    pub times: u32,
}
/// Binary echo response.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BinaryEchoResponse {
    pub data: Vec<u8>,
}
