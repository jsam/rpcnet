use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;
use std::time::Duration;

// Core RPC error types
#[derive(Debug, Error)]
pub enum RpcError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Transport error: {0}")]
    TransportError(#[from] s2n_quic::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),
    
    #[error("Request timeout")]
    Timeout,
    
    #[error("Unknown method: {0}")]
    UnknownMethod(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}