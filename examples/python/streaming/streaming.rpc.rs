use serde::{Deserialize, Serialize};
use futures::Stream;
use std::pin::Pin;

/// Request for unary (single request, single response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnaryRequest {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnaryResponse {
    pub reply: String,
    pub timestamp: i64,
}

/// Request for server streaming (single request, stream of responses)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStreamRequest {
    pub count: u32,
    pub prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStreamResponse {
    pub item: String,
    pub index: u32,
}

/// Request for client streaming (stream of requests, single response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientStreamRequest {
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientStreamResponse {
    pub sum: i64,
    pub count: u32,
}

/// Request for bidirectional streaming (stream of requests, stream of responses)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BidiStreamRequest {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BidiStreamResponse {
    pub echo: String,
    pub reversed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamingError {
    InvalidInput(String),
    ProcessingError(String),
}

#[rpcnet::service]
pub trait StreamingService {
    /// Unary: Single request -> Single response
    async fn unary(&self, request: UnaryRequest) -> Result<UnaryResponse, StreamingError>;
    
    /// Server streaming: Single request -> Stream of responses
    async fn server_stream(
        &self,
        request: ServerStreamRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = ServerStreamResponse> + Send>>, StreamingError>;
    
    /// Client streaming: Stream of requests -> Single response
    async fn client_stream(
        &self,
        request_stream: Pin<Box<dyn Stream<Item = ClientStreamRequest> + Send>>,
    ) -> Result<ClientStreamResponse, StreamingError>;
    
    /// Bidirectional streaming: Stream of requests -> Stream of responses
    async fn bidi_stream(
        &self,
        request_stream: Pin<Box<dyn Stream<Item = BidiStreamRequest> + Send>>,
    ) -> Result<Pin<Box<dyn Stream<Item = BidiStreamResponse> + Send>>, StreamingError>;
}
