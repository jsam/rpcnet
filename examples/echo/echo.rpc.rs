//! Echo service definition.

use serde::{Serialize, Deserialize};

/// Request for echo operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EchoRequest {
    pub message: String,
    pub times: u32,
}

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

/// Binary echo response.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BinaryEchoResponse {
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

/// Echo service for testing communication.
#[rpcnet::service]
pub trait Echo {
    async fn echo(&self, request: EchoRequest) -> Result<EchoResponse, EchoError>;
    async fn binary_echo(&self, request: BinaryEchoRequest) -> Result<BinaryEchoResponse, EchoError>;
}