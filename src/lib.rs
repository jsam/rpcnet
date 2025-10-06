use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use futures::{Stream, StreamExt};
use s2n_quic::{client::Connect, connection::Connection as QuicConnection, Client};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    future::Future,
    net::SocketAddr,
    path::Path,
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use thiserror::Error;
use tokio::{
    sync::{oneshot, RwLock},
    task::JoinHandle,
};
use tracing::debug;

pub mod streaming;

pub mod runtime {
    //! Helpers for configuring Tokio runtimes.
    use std::{env, thread};

    /// Environment variable that controls the number of server worker threads.
    pub const SERVER_THREADS_ENV: &str = "RPCNET_SERVER_THREADS";

    /// Returns the worker thread count derived from [`SERVER_THREADS_ENV`],
    /// falling back to the number of available CPU threads.
    pub fn server_worker_threads() -> usize {
        threads_from_env(SERVER_THREADS_ENV).unwrap_or_else(default_worker_threads)
    }

    /// Parses an environment variable as a positive worker-thread count.
    pub fn threads_from_env(key: &str) -> Option<usize> {
        let raw = env::var(key).ok()?;
        parse_threads(&raw)
    }

    fn default_worker_threads() -> usize {
        thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }

    fn parse_threads(raw: &str) -> Option<usize> {
        let value = raw.trim().parse::<usize>().ok()?;
        (value > 0).then_some(value)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parse_threads_rejects_invalid_values() {
            assert_eq!(parse_threads("0"), None);
            assert_eq!(parse_threads("-1"), None);
            assert_eq!(parse_threads("abc"), None);
        }

        #[test]
        fn parse_threads_accepts_positive_values() {
            assert_eq!(parse_threads("8"), Some(8));
            assert_eq!(parse_threads(" 4 "), Some(4));
        }
    }
}

pub mod cluster;

#[cfg(feature = "codegen")]
pub mod codegen;

#[cfg(not(test))]
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(test)]
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("TLS error: {0}")]
    TlsError(String),

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

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Invalid migration token")]
    InvalidToken,

    #[error("Migration rejected")]
    MigrationRejected,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest {
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

#[derive(Debug, Serialize, Deserialize)]
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
        match result {
            Ok(data) => Self::new(id, Some(data), None),
            Err(e) => Self::new(id, None, Some(e.to_string())),
        }
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

#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub cert_path: PathBuf,

    pub key_path: Option<PathBuf>,

    pub server_name: String,

    pub bind_address: String,

    pub keep_alive_interval: Option<Duration>,

    pub default_stream_timeout: Duration,
}

impl RpcConfig {
    pub fn new<P: Into<PathBuf>>(cert_path: P, bind_address: impl Into<String>) -> Self {
        Self {
            cert_path: cert_path.into(),
            key_path: None,
            server_name: "localhost".to_string(),
            bind_address: bind_address.into(),
            keep_alive_interval: Some(Duration::from_secs(30)),
            default_stream_timeout: Duration::from_secs(3),
        }
    }

    pub fn with_key_path<P: Into<PathBuf>>(mut self, key_path: P) -> Self {
        self.key_path = Some(key_path.into());
        self
    }

    pub fn with_server_name(mut self, server_name: impl Into<String>) -> Self {
        self.server_name = server_name.into();
        self
    }

    pub fn with_keep_alive_interval(mut self, interval: Duration) -> Self {
        self.keep_alive_interval = Some(interval);
        self
    }

    pub fn with_default_stream_timeout(mut self, timeout: Duration) -> Self {
        self.default_stream_timeout = timeout;
        self
    }
}

type AsyncHandlerFn = Box<
    dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, RpcError>> + Send>>
        + Send
        + Sync,
>;

type AsyncStreamingHandlerFn = Box<
    dyn Fn(
            Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>,
        ) -> Pin<
            Box<
                dyn Future<Output = Pin<Box<dyn Stream<Item = Result<Vec<u8>, RpcError>> + Send>>>
                    + Send,
            >,
        > + Send
        + Sync,
>;

#[derive(Clone)]
pub struct RpcServer {
    pub handlers: Arc<RwLock<HashMap<String, AsyncHandlerFn>>>,

    pub streaming_handlers: Arc<RwLock<HashMap<String, AsyncStreamingHandlerFn>>>,

    pub socket_addr: Option<SocketAddr>,

    pub config: RpcConfig,

    cluster: Arc<RwLock<Option<Arc<cluster::ClusterMembership>>>>,
}

#[derive(Debug)]
pub enum ConnectionDriveOutcome {
    /// The client closed the connection while being served by the current worker.
    ConnectionClosed,
    /// The worker was asked to hand off the live connection to another worker.
    HandoffReady(QuicConnection),
}

#[async_trait]
pub(crate) trait QuicStreamAdapter: Send {
    async fn send_bytes(&mut self, data: Bytes) -> Result<(), RpcError>;
    async fn receive_bytes(&mut self) -> Result<Option<Bytes>, RpcError>;
}

#[async_trait]
impl QuicStreamAdapter for s2n_quic::stream::BidirectionalStream {
    async fn send_bytes(&mut self, data: Bytes) -> Result<(), RpcError> {
        self.send(data)
            .await
            .map_err(|err| RpcError::StreamError(err.to_string()))
    }

    async fn receive_bytes(&mut self) -> Result<Option<Bytes>, RpcError> {
        self.receive()
            .await
            .map_err(|err| RpcError::StreamError(err.to_string()))
    }
}

#[async_trait]
impl<T> QuicStreamAdapter for Box<T>
where
    T: QuicStreamAdapter + ?Sized,
{
    async fn send_bytes(&mut self, data: Bytes) -> Result<(), RpcError> {
        (**self).send_bytes(data).await
    }

    async fn receive_bytes(&mut self) -> Result<Option<Bytes>, RpcError> {
        (**self).receive_bytes().await
    }
}

#[async_trait]
pub(crate) trait QuicConnectionAdapter: Send + Sync {
    async fn open_bidirectional_stream(
        &mut self,
    ) -> Result<Box<dyn QuicStreamAdapter + Send>, RpcError>;
}

pub(crate) struct RealConnectionAdapter {
    inner: s2n_quic::Connection,
}

impl RealConnectionAdapter {
    fn new(inner: s2n_quic::Connection) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl QuicConnectionAdapter for RealConnectionAdapter {
    async fn open_bidirectional_stream(
        &mut self,
    ) -> Result<Box<dyn QuicStreamAdapter + Send>, RpcError> {
        let stream = self
            .inner
            .open_bidirectional_stream()
            .await
            .map_err(|e| RpcError::StreamError(e.to_string()))?;
        Ok(Box::new(stream))
    }
}

#[async_trait]
pub(crate) trait QuicServerConnectionAdapter: Send {
    async fn accept_bidirectional_stream(
        &mut self,
    ) -> Result<Option<Box<dyn QuicStreamAdapter + Send>>, RpcError>;
}

pub(crate) struct RealServerConnectionAdapter {
    inner: s2n_quic::connection::Connection,
}

impl RealServerConnectionAdapter {
    fn new(inner: s2n_quic::connection::Connection) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl QuicServerConnectionAdapter for RealServerConnectionAdapter {
    async fn accept_bidirectional_stream(
        &mut self,
    ) -> Result<Option<Box<dyn QuicStreamAdapter + Send>>, RpcError> {
        match self.inner.accept_bidirectional_stream().await {
            Ok(Some(stream)) => Ok(Some(Box::new(stream))),
            Ok(None) => Ok(None),
            Err(e) => Err(RpcError::StreamError(e.to_string())),
        }
    }
}

#[async_trait]
pub(crate) trait QuicServerAdapter: Send {
    async fn accept(&mut self) -> Option<Box<dyn QuicServerConnectionAdapter>>;
}

pub(crate) struct RealServerAdapter {
    inner: s2n_quic::Server,
}

impl RealServerAdapter {
    fn new(inner: s2n_quic::Server) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl QuicServerAdapter for RealServerAdapter {
    async fn accept(&mut self) -> Option<Box<dyn QuicServerConnectionAdapter>> {
        self.inner
            .accept()
            .await
            .map(|connection| Box::new(RealServerConnectionAdapter::new(connection)) as _)
    }
}

impl RpcServer {
    pub fn new(config: RpcConfig) -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            streaming_handlers: Arc::new(RwLock::new(HashMap::new())),
            socket_addr: None,
            config,
            cluster: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn register<F, Fut>(&self, method: &str, handler: F)
    where
        F: Fn(Vec<u8>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<u8>, RpcError>> + Send + 'static,
    {
        let mut handlers = self.handlers.write().await;
        handlers.insert(
            method.to_string(),
            Box::new(move |params: Vec<u8>| {
                Box::pin(handler(params)) as Pin<Box<dyn Future<Output = _> + Send>>
            }),
        );
    }

    pub async fn register_typed<Req, Resp, F, Fut>(&self, method: &str, handler: F)
    where
        Req: serde::de::DeserializeOwned + Send + 'static,
        Resp: serde::Serialize + Send + 'static,
        F: Fn(Req) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Resp, RpcError>> + Send + 'static,
    {
        let handler = Arc::new(handler);
        self.register(method, move |params: Vec<u8>| {
            let handler = handler.clone();
            async move {
                let request: Req = bincode::deserialize(&params)
                    .map_err(|e| RpcError::SerializationError(e))?;
                
                let response = handler(request).await?;
                
                bincode::serialize(&response)
                    .map_err(|e| RpcError::SerializationError(e))
            }
        }).await;
    }

    pub async fn register_streaming<F, Fut, S>(&self, method: &str, handler: F)
    where
        F: Fn(Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = S> + Send + 'static,
        S: Stream<Item = Result<Vec<u8>, RpcError>> + Send + 'static,
    {
        let mut handlers = self.streaming_handlers.write().await;
        handlers.insert(
            method.to_string(),
            Box::new(move |request_stream| {
                let handler = handler.clone();
                Box::pin(async move {
                    let response_stream = handler(request_stream).await;
                    Box::pin(response_stream)
                        as Pin<Box<dyn Stream<Item = Result<Vec<u8>, RpcError>> + Send>>
                })
            }),
        );
    }

    pub async fn start(&mut self, server: s2n_quic::Server) -> Result<(), RpcError> {
        let mut adapter = RealServerAdapter::new(server);
        self.start_with_adapter(&mut adapter).await
    }

    async fn start_with_adapter<A>(&mut self, server: &mut A) -> Result<(), RpcError>
    where
        A: QuicServerAdapter,
    {
        while let Some(mut connection) = server.accept().await {
            let handlers = self.handlers.clone();
            let streaming_handlers = self.streaming_handlers.clone();
            let cluster = self.cluster.clone();

            tokio::spawn(async move {
                // For each accepted connection, keep accepting streams:
                while let Ok(Some(stream)) = connection.accept_bidirectional_stream().await {
                    let handlers = handlers.clone();
                    let streaming_handlers = streaming_handlers.clone();
                    let cluster = cluster.clone();

                    tokio::spawn(Self::handle_stream(handlers, streaming_handlers, cluster, stream));
                }
            });
        }

        Ok(())
    }

    async fn handle_stream(
        handlers: Arc<RwLock<HashMap<String, AsyncHandlerFn>>>,
        streaming_handlers: Arc<RwLock<HashMap<String, AsyncStreamingHandlerFn>>>,
        cluster: Arc<RwLock<Option<Arc<cluster::ClusterMembership>>>>,
        stream: Box<dyn QuicStreamAdapter + Send>,
    ) {
        let stream = Arc::new(tokio::sync::Mutex::new(stream));
        let mut request_data = Vec::with_capacity(8192);

        loop {
            let chunk = {
                let mut stream_guard = stream.lock().await;
                stream_guard.receive_bytes().await
            };

            let chunk = match chunk {
                Ok(Some(bytes)) => {
                    debug!("📦 Received {} bytes on stream", bytes.len());
                    bytes
                },
                Ok(None) => {
                    debug!("🔚 Stream closed");
                    break;
                },
                Err(e) => {
                    debug!("❌ Stream error: {:?}", e);
                    break;
                }
            };

            request_data.extend_from_slice(&chunk);
            debug!("📊 Total request_data size: {} bytes", request_data.len());

            // First, try to parse as SWIM gossip message
            match cluster::gossip::SwimMessage::deserialize(&request_data) {
                Ok(swim_msg) => {
                    debug!("✅ Successfully deserialized SWIM message!");
                    if let Some(cluster_membership) = cluster.read().await.as_ref() {
                        Self::handle_swim_message(cluster_membership, swim_msg, &stream).await;
                    } else {
                        debug!("⚠️  Received SWIM message but cluster not enabled");
                    }
                    break;
                },
                Err(e) => {
                    debug!("⚠️  Not a SWIM message (tried {} bytes): {:?}", request_data.len(), e);
                }
            }

            // Then try to parse as regular RPC request (original behavior)
            if let Ok(request) = bincode::deserialize::<RpcRequest>(&request_data) {
                debug!("📨 Received RPC request: {}", request.method);
                let handlers = handlers.read().await;
                let response = match handlers.get(request.method()) {
                    Some(handler) => {
                        let result = handler(request.params().to_vec()).await;
                        RpcResponse::from_result(request.id(), result)
                    }
                    None => RpcResponse::new(
                        request.id(),
                        None,
                        Some(format!("Unknown method: {}", request.method())),
                    ),
                };
                if let Ok(response_data) = bincode::serialize(&response) {
                    let mut stream_guard = stream.lock().await;
                    let _ = stream_guard.send_bytes(Bytes::from(response_data)).await;
                }
                break; // Handle one request per stream
            }

            // If regular RPC parsing fails and we have enough data, check for streaming protocol
            if request_data.len() >= 4 {
                let method_len = u32::from_le_bytes([
                    request_data[0],
                    request_data[1],
                    request_data[2],
                    request_data[3],
                ]) as usize;

                // Validate method length is reasonable (prevent huge allocations)
                if method_len > 0 && method_len < 1024 && request_data.len() >= 4 + method_len {
                    if let Ok(method_name) = std::str::from_utf8(&request_data[4..4 + method_len]) {
                        // Check if this is a known streaming method
                        let streaming_handlers_ref = streaming_handlers.read().await;
                        if streaming_handlers_ref.contains_key(method_name) {
                            drop(streaming_handlers_ref); // Release the read lock

                            // Create stream with remaining data after method name
                            let remaining_data = request_data[4 + method_len..].to_owned();
                            let stream_arc = stream.clone();
                            let request_stream = Self::create_request_stream_with_initial_data(
                                stream_arc.clone(),
                                remaining_data,
                            );

                            let streaming_handlers_ref = streaming_handlers.read().await;
                            if let Some(handler) = streaming_handlers_ref.get(method_name) {
                                let response_stream = handler(request_stream).await;
                                Self::send_response_stream(stream_arc, response_stream).await;
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn handle_swim_message(
        cluster: &Arc<cluster::ClusterMembership>,
        msg: cluster::gossip::SwimMessage,
        stream: &Arc<tokio::sync::Mutex<Box<dyn QuicStreamAdapter + Send>>>,
    ) {
        use cluster::gossip::{NodeUpdate, SwimMessage};
        use cluster::incarnation::NodeStatus;
        use cluster::node_registry::NodeRegistry;
        use std::time::Instant;

        debug!("🔔 [SWIM] Received message: {:?}", msg);
        let update_count = msg.updates().len();
        debug!("🔔 [SWIM] Processing {} node updates", update_count);

        for update in msg.updates() {
            debug!("🔔 [SWIM] Update: node_id={:?}, addr={}, state={:?}, tags={:?}", 
                  update.node_id, update.addr, update.state, update.tags);
            let node_status = NodeStatus {
                node_id: update.node_id.clone(),
                addr: update.addr,
                incarnation: update.incarnation,
                state: update.state.clone(),
                last_seen: Instant::now(),
                tags: update.tags.clone(),
            };
            cluster.registry().insert(node_status.clone());
            debug!("✅ [SWIM] Inserted {:?} into registry with tags: {:?}", 
                  node_status.node_id, node_status.tags);
        }

        let self_status = cluster.registry().get(cluster.node_id());
        let my_updates = if let Some(status) = self_status {
            vec![NodeUpdate {
                node_id: status.node_id,
                addr: status.addr,
                incarnation: status.incarnation,
                state: status.state,
                tags: status.tags,
            }]
        } else {
            vec![]
        };

        let response = match msg {
            SwimMessage::Ping { from, seq, .. } => SwimMessage::Ack {
                from: cluster.node_id().clone(),
                to: from,
                updates: my_updates,
                seq,
            },
            SwimMessage::PingReq { .. } => {
                return;
            }
            SwimMessage::Ack { .. } => {
                return;
            }
        };

        if let Ok(response_bytes) = response.serialize() {
            let mut stream_guard = stream.lock().await;
            let _ = stream_guard.send_bytes(Bytes::from(response_bytes)).await;
        }
    }

    /// Drives an existing QUIC connection until it is either closed by the client
    /// or a shutdown signal requests that it be handed off to another worker.
    pub async fn drive_connection(
        &self,
        mut connection: QuicConnection,
        mut shutdown: oneshot::Receiver<()>,
    ) -> Result<ConnectionDriveOutcome, RpcError> {
        let handlers = self.handlers.clone();
        let streaming_handlers = self.streaming_handlers.clone();
        let cluster = self.cluster.clone();
        let mut stream_tasks: Vec<JoinHandle<()>> = Vec::new();

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    for task in stream_tasks {
                        task.abort();
                    }
                    return Ok(ConnectionDriveOutcome::HandoffReady(connection));
                }
                accept_result = connection.accept_bidirectional_stream() => {
                    match accept_result {
                        Ok(Some(stream)) => {
                            let handlers = handlers.clone();
                            let streaming_handlers = streaming_handlers.clone();
                            let cluster = cluster.clone();
                            let task = tokio::spawn(Self::handle_stream(
                                handlers,
                                streaming_handlers,
                                cluster,
                                Box::new(stream) as Box<dyn QuicStreamAdapter + Send>,
                            ));
                            stream_tasks.push(task);
                        }
                        Ok(None) => {
                            for task in stream_tasks {
                                let _ = task.await;
                            }
                            return Ok(ConnectionDriveOutcome::ConnectionClosed);
                        }
                        Err(err) => {
                            for task in stream_tasks {
                                task.abort();
                            }
                            return Err(RpcError::StreamError(err.to_string()));
                        }
                    }
                }
            }
        }
    }

    fn create_request_stream_with_initial_data<S>(
        stream: Arc<tokio::sync::Mutex<S>>,
        initial_data: Vec<u8>,
    ) -> Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>
    where
        S: QuicStreamAdapter + 'static,
    {
        Box::pin(async_stream::stream! {
            let mut buffer = BytesMut::with_capacity(8192 + initial_data.len());
            buffer.extend_from_slice(&initial_data);

            loop {
                // First try to parse any complete messages from existing buffer
                while buffer.len() >= 4 {
                    let len = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;

                    if len == 0 {
                        // End of stream marker
                        return;
                    }

                    if buffer.len() >= 4 + len {
                        // We have a complete message
                        let message_data = buffer.split_to(4 + len);
                        let request_data = message_data[4..].to_owned();
                        yield request_data;
                    } else {
                        // Need more data
                        break;
                    }
                }

                // If we need more data, receive from stream
                let chunk = {
                    let mut stream_guard = stream.lock().await;
                    stream_guard.receive_bytes().await
                };

                match chunk {
                    Ok(Some(chunk)) => buffer.extend_from_slice(&chunk),
                    _ => break, // Connection closed or error
                }
            }
        })
    }

    fn create_request_stream<S>(
        stream: Arc<tokio::sync::Mutex<S>>,
    ) -> Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>
    where
        S: QuicStreamAdapter + 'static,
    {
        Box::pin(async_stream::stream! {
            let mut buffer = BytesMut::with_capacity(8192);

            loop {
                let chunk = {
                    let mut stream_guard = stream.lock().await;
                    stream_guard.receive_bytes().await
                };

                match chunk {
                    Ok(Some(chunk)) => {
                        buffer.extend_from_slice(&chunk);

                        // Parse length-prefixed messages
                        while buffer.len() >= 4 {
                            let len = u32::from_le_bytes([
                                buffer[0],
                                buffer[1],
                                buffer[2],
                                buffer[3],
                            ]) as usize;

                            if len == 0 {
                                // End of stream marker
                                return;
                            }

                            if buffer.len() >= 4 + len {
                                // We have a complete message
                                let message_data = buffer.split_to(4 + len);
                                let request_data = message_data[4..].to_owned();
                                yield request_data;
                            } else {
                                // Need more data
                                break;
                            }
                        }
                    }
                    _ => break, // Connection closed or error
                }
            }
        })
    }

    async fn send_response_stream<S>(
        stream: Arc<tokio::sync::Mutex<S>>,
        mut response_stream: Pin<Box<dyn Stream<Item = Result<Vec<u8>, RpcError>> + Send>>,
    ) where
        S: QuicStreamAdapter + 'static,
    {
        while let Some(response_result) = response_stream.next().await {
            match response_result {
                Ok(response_data) => {
                    let data_len = (response_data.len() as u32).to_le_bytes();
                    let mut stream_guard = stream.lock().await;
                    let mut payload = Vec::with_capacity(4 + response_data.len());
                    payload.extend_from_slice(&data_len);
                    payload.extend_from_slice(&response_data);
                    if stream_guard.send_bytes(Bytes::from(payload)).await.is_err() {
                        break;
                    }
                }
                Err(_) => {
                    // Send error and continue
                    let error_data = b"Error processing request";
                    let data_len = (error_data.len() as u32).to_le_bytes();
                    let mut stream_guard = stream.lock().await;
                    let mut payload = Vec::with_capacity(4 + error_data.len());
                    payload.extend_from_slice(&data_len);
                    payload.extend_from_slice(error_data);
                    if stream_guard.send_bytes(Bytes::from(payload)).await.is_err() {
                        break;
                    }
                }
            }
        }

        // Send end-of-stream marker
        let mut stream_guard = stream.lock().await;
        let _ = stream_guard
            .send_bytes(Bytes::from_static(&[0, 0, 0, 0]))
            .await;
    }

    pub fn bind(&mut self) -> Result<s2n_quic::Server, RpcError> {
        let cert_path = canonicalize_path(&self.config.cert_path)?;
        let key_path =
            self.config.key_path.as_ref().ok_or_else(|| {
                RpcError::ConfigError("Server key path not configured".to_string())
            })?;
        let key_path = canonicalize_path(key_path)?;

        // Create optimized limits for high-performance RPC
        let limits = s2n_quic::provider::limits::Limits::new()
            // Increase stream limits for high concurrency
            .with_max_open_local_bidirectional_streams(10_000)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set stream limits: {:?}", e)))?
            .with_max_open_remote_bidirectional_streams(10_000)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set stream limits: {:?}", e)))?
            // Increase data windows for better throughput (16MB connection, 8MB per stream)
            .with_data_window(16 * 1024 * 1024)
            .map_err(|e| RpcError::ConfigError(format!("Failed to set data window: {:?}", e)))?
            .with_bidirectional_local_data_window(8 * 1024 * 1024)
            .map_err(|e| {
                RpcError::ConfigError(format!("Failed to set bidirectional window: {:?}", e))
            })?
            // Optimize for local network performance
            .with_initial_round_trip_time(Duration::from_millis(1)) // Low RTT for local connections
            .map_err(|e| RpcError::ConfigError(format!("Failed to set RTT: {:?}", e)))?
            .with_max_ack_delay(Duration::from_millis(5)) // Faster ACK responses
            .map_err(|e| RpcError::ConfigError(format!("Failed to set ACK delay: {:?}", e)))?
            // Increase send buffer for better performance
            .with_max_send_buffer_size(2 * 1024 * 1024) // 2MB send buffer
            .map_err(|e| RpcError::ConfigError(format!("Failed to set send buffer: {:?}", e)))?;

        let server = s2n_quic::Server::builder()
            .with_tls((cert_path.as_path(), key_path.as_path()))
            .map_err(|e| RpcError::TlsError(format!("{:?}", e)))?
            .with_limits(limits)
            .map_err(|e| RpcError::ConfigError(format!("Failed to apply limits: {:?}", e)))?
            .with_io(self.config.bind_address.as_str())
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?
            .start()
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?;

        let local_addr = server.local_addr().map_err(|_err| {
            RpcError::ConfigError("Could not retrieve local_addr() from server".to_string())
        })?;

        self.socket_addr = Some(local_addr);
        println!("RPC server listening on {local_addr}");
        Ok(server)
    }

    pub async fn enable_cluster(
        &self,
        config: cluster::ClusterConfig,
        seeds: Vec<SocketAddr>,
        quic_client: Arc<Client>,
    ) -> Result<(), cluster::ClusterError> {
        let addr = self
            .socket_addr
            .ok_or_else(|| cluster::ClusterError::BootstrapTimeout(Duration::from_secs(0)))?;

        let membership = Arc::new(cluster::ClusterMembership::new(addr, config, quic_client).await?);
        membership.join(seeds).await?;

        let mut cluster_guard = self.cluster.write().await;
        *cluster_guard = Some(membership);

        Ok(())
    }

    pub async fn cluster(&self) -> Option<Arc<cluster::ClusterMembership>> {
        self.cluster.read().await.clone()
    }

    pub async fn update_tag(&self, key: String, value: String) -> Result<(), RpcError> {
        if let Some(cluster) = self.cluster().await {
            cluster.update_tag(key, value).await;
            Ok(())
        } else {
            Err(RpcError::ConfigError("Cluster not enabled".to_string()))
        }
    }

    pub async fn cluster_events(&self) -> Option<cluster::ClusterEventReceiver> {
        self.cluster().await.map(|c| c.subscribe())
    }
}

fn canonicalize_path(path: &Path) -> Result<std::path::PathBuf, RpcError> {
    fs::canonicalize(path).map_err(|e| {
        RpcError::ConfigError(format!("Failed to canonicalize {}: {e}", path.display()))
    })
}

pub struct RpcClient {
    connection: Arc<RwLock<Box<dyn QuicConnectionAdapter + Send + Sync>>>,
    config: RpcConfig,
    pub next_id: Arc<AtomicU64>,
}

impl RpcClient {
    pub async fn connect(connect_addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        // Create optimized limits matching server configuration
        let limits = s2n_quic::provider::limits::Limits::new()
            .with_max_open_local_bidirectional_streams(10_000)
            .map_err(|e| {
                RpcError::ConfigError(format!("Failed to set client stream limits: {:?}", e))
            })?
            .with_max_open_remote_bidirectional_streams(10_000)
            .map_err(|e| {
                RpcError::ConfigError(format!("Failed to set client stream limits: {:?}", e))
            })?
            .with_data_window(16 * 1024 * 1024)
            .map_err(|e| {
                RpcError::ConfigError(format!("Failed to set client data window: {:?}", e))
            })?
            .with_bidirectional_local_data_window(8 * 1024 * 1024)
            .map_err(|e| {
                RpcError::ConfigError(format!(
                    "Failed to set client bidirectional window: {:?}",
                    e
                ))
            })?
            .with_initial_round_trip_time(Duration::from_millis(1))
            .map_err(|e| RpcError::ConfigError(format!("Failed to set client RTT: {:?}", e)))?
            .with_max_ack_delay(Duration::from_millis(5))
            .map_err(|e| RpcError::ConfigError(format!("Failed to set client ACK delay: {:?}", e)))?
            .with_max_send_buffer_size(2 * 1024 * 1024)
            .map_err(|e| {
                RpcError::ConfigError(format!("Failed to set client send buffer: {:?}", e))
            })?;

        let client = Client::builder()
            .with_tls(config.cert_path.as_path())
            .map_err(|e| RpcError::TlsError(format!("{:?}", e)))?
            .with_limits(limits)
            .map_err(|e| RpcError::ConfigError(format!("Failed to apply client limits: {:?}", e)))?
            .with_io(config.bind_address.as_str())
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?
            .start()
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?;

        let connect = Connect::new(connect_addr).with_server_name(config.server_name.as_str());
        let mut connection = client
            .connect(connect)
            .await
            .map_err(|e| RpcError::ConnectionError(e.to_string()))?;

        if let Some(_interval) = config.keep_alive_interval {
            connection
                .keep_alive(true)
                .map_err(|e| RpcError::ConfigError(e.to_string()))?;
        }

        Ok(Self {
            connection: Arc::new(RwLock::new(Box::new(RealConnectionAdapter::new(
                connection,
            )))),
            config,
            next_id: Arc::new(AtomicU64::new(1)),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_connection(
        connection: Box<dyn QuicConnectionAdapter + Send + Sync>,
    ) -> Self {
        Self {
            connection: Arc::new(RwLock::new(connection)),
            config: RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0"),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub async fn call(&self, method: &str, params: Vec<u8>) -> Result<Vec<u8>, RpcError> {
        // Generate a new request ID
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = RpcRequest::new(id, method.to_string(), params);
        // Pre-allocate serialization buffer to avoid reallocations
        let req_data = bincode::serialize(&req)?;

        // Open a new bidirectional stream with minimal lock time
        let mut stream = {
            let mut conn = self.connection.write().await;
            conn.open_bidirectional_stream().await?
        }; // Lock released immediately after stream creation

        // Send the request
        stream.send_bytes(Bytes::from(req_data)).await?;

        // Read back the response with optimized buffering
        let read_future = async {
            // Use BytesMut for more efficient buffer management
            let mut response_data = BytesMut::with_capacity(1024);
            while let Ok(Some(chunk)) = stream.receive_bytes().await {
                response_data.extend_from_slice(&chunk);

                // Only attempt deserialization if we have a reasonable amount of data
                if response_data.len() >= 16 {
                    // Minimum for a valid response
                    if let Ok(response) = bincode::deserialize::<RpcResponse>(&response_data[..]) {
                        if response.id() == id {
                            // Extract data without cloning when possible
                            return match (response.result(), response.error()) {
                                (Some(data), None) => Ok(data.to_vec()), // More explicit about the copy
                                (None, Some(err_msg)) => {
                                    Err(RpcError::StreamError(err_msg.to_string()))
                                } // Already owned
                                _ => Err(RpcError::StreamError("Invalid response".into())), // Avoid string allocation
                            };
                        }
                    }
                }
            }
            // If we exit the loop without returning, the stream closed early or never gave a valid response
            Err(RpcError::ConnectionError(
                "Stream closed unexpectedly".into(),
            ))
        };

        // Enforce the DEFAULT_TIMEOUT
        match tokio::time::timeout(DEFAULT_TIMEOUT, read_future).await {
            Ok(res) => res,
            Err(_) => Err(RpcError::Timeout),
        }
    }

    pub async fn call_streaming<S>(
        &self,
        method: &str,
        request_stream: S,
    ) -> Result<streaming::TimeoutStream<impl Stream<Item = Result<Vec<u8>, RpcError>>>, RpcError>
    where
        S: Stream<Item = Vec<u8>> + Send + 'static,
    {
        // Open a new bidirectional stream
        let mut stream = {
            let mut conn = self.connection.write().await;
            conn.open_bidirectional_stream().await?
        };

        // Send the method name first (with length prefix)
        let method_data = method.as_bytes();
        let method_len = (method_data.len() as u32).to_le_bytes();
        stream
            .send_bytes(Bytes::from([&method_len[..], method_data].concat()))
            .await?;

        // Use a single task to handle BOTH send and receive with select! - NO LOCKS!
        // This enables true concurrent bidirectional streaming without mutex contention
        // The key insight: use tokio::select! to multiplex send/receive on the same task
        
        // Channel to deliver responses back to the caller
        let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel::<Result<Vec<u8>, RpcError>>();
        
        // Spawn a single task that owns the stream and multiplexes send/receive
        let mut request_stream = Box::pin(request_stream);
        tokio::spawn(async move {
            let mut buffer = BytesMut::with_capacity(8192);
            let mut send_done = false;
            let mut recv_done = false;
            
            loop {
                tokio::select! {
                    // Handle sending requests
                    request_opt = request_stream.next(), if !send_done => {
                        match request_opt {
                            Some(request_data) => {
                                let data_len = (request_data.len() as u32).to_le_bytes();
                                // Even if send fails, try to send end marker before breaking
                                if stream.send_bytes(Bytes::from([&data_len[..], &request_data].concat())).await.is_err() {
                                    // Try to send end marker before giving up
                                    let _ = stream.send_bytes(Bytes::from(vec![0, 0, 0, 0])).await;
                                    break;
                                }
                            }
                            None => {
                                if let Err(_) = stream.send_bytes(Bytes::from(vec![0, 0, 0, 0])).await {
                                    break;
                                }
                                send_done = true;
                                if recv_done {
                                    break;
                                }
                            }
                        }
                    }
                    
                    // Handle receiving responses (happens concurrently with sending!)
                    chunk_result = stream.receive_bytes(), if !recv_done => {
                        match chunk_result {
                            Ok(Some(chunk)) => {
                                buffer.extend_from_slice(&chunk);
                                
                                // Parse complete messages
                                while buffer.len() >= 4 {
                                    let len = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
                                    
                                    if len == 0 {
                                        recv_done = true;
                                        if send_done {
                                            return;
                                        }
                                        break;
                                    }
                                    
                                    if buffer.len() >= 4 + len {
                                        let message_data = buffer.split_to(4 + len);
                                        let response_data = message_data[4..].to_vec();
                                        if response_tx.send(Ok(response_data)).is_err() {
                                            return;
                                        }
                                    } else {
                                        break;
                                    }
                                }
                            }
                            Ok(None) => {
                                break;
                            }
                            Err(_e) => {
                                break;
                            }
                        }
                    }
                }
            }
        });

        // Create the base stream
        let base_stream = async_stream::stream! {
            while let Some(response) = response_rx.recv().await {
                yield response;
            }
        };

        // Wrap with timeout stream using config's default timeout
        Ok(streaming::TimeoutStream::new(base_stream, self.config.default_stream_timeout))
    }

    pub async fn call_server_streaming(
        &self,
        method: &str,
        request: Vec<u8>,
    ) -> Result<streaming::TimeoutStream<impl Stream<Item = Result<Vec<u8>, RpcError>>>, RpcError> {
        use futures::stream;

        // Create a single-item stream for the request
        let request_stream = stream::iter(vec![request]);

        // Use the bidirectional streaming method
        self.call_streaming(method, request_stream).await
    }

    pub async fn call_client_streaming<S>(
        &self,
        method: &str,
        request_stream: S,
    ) -> Result<Vec<u8>, RpcError>
    where
        S: Stream<Item = Vec<u8>> + Send + 'static,
    {
        // Use the bidirectional streaming method and collect the first response
        let response_stream = self.call_streaming(method, request_stream).await?;
        let mut response_stream = Box::pin(response_stream);

        match response_stream.next().await {
            Some(Ok(response)) => Ok(response),
            Some(Err(streaming::StreamError::Timeout)) => Err(RpcError::Timeout),
            Some(Err(streaming::StreamError::Transport(e))) => Err(e),
            Some(Err(streaming::StreamError::Item(_))) => {
                Err(RpcError::StreamError("Unexpected item error".to_string()))
            }
            None => Err(RpcError::StreamError("No response received".to_string())),
        }
    }
}

#[cfg(test)]
mod rpc_response_tests {
    use super::{RpcError, RpcResponse};

    #[test]
    fn from_result_success() {
        let resp = RpcResponse::from_result(42, Ok(b"ok".to_vec()));
        assert_eq!(resp.id(), 42);
        assert_eq!(resp.result(), Some(&b"ok".to_vec()));
        assert!(resp.error().is_none());
    }

    #[test]
    fn from_result_error() {
        let resp = RpcResponse::from_result(7, Err(RpcError::StreamError("boom".into())));
        assert_eq!(resp.id(), 7);
        assert!(resp.result().is_none());
        assert_eq!(resp.error(), Some(&"Stream error: boom".to_string()));
    }
}

#[cfg(test)]
mod streaming_helper_tests {
    use super::{QuicStreamAdapter, RpcError, RpcServer};
    use async_trait::async_trait;
    use bytes::Bytes;
    use futures::{stream, StreamExt};
    use std::collections::VecDeque;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct MockStream {
        pending: VecDeque<Result<Option<Vec<u8>>, RpcError>>,
        sent: Vec<Vec<u8>>,
        send_error_after: Option<usize>,
        send_calls: usize,
    }

    impl MockStream {
        fn new(pending: VecDeque<Result<Option<Vec<u8>>, RpcError>>) -> Self {
            Self {
                pending,
                sent: Vec::new(),
                send_error_after: None,
                send_calls: 0,
            }
        }

        fn with_send_error_after(mut self, after: usize) -> Self {
            self.send_error_after = Some(after);
            self
        }
    }

    #[async_trait]
    impl QuicStreamAdapter for MockStream {
        async fn send_bytes(&mut self, data: Bytes) -> Result<(), RpcError> {
            if let Some(limit) = self.send_error_after {
                if self.send_calls >= limit {
                    return Err(RpcError::StreamError("mock send failure".to_string()));
                }
            }

            self.send_calls += 1;
            self.sent.push(data.to_vec());
            Ok(())
        }

        async fn receive_bytes(&mut self) -> Result<Option<Bytes>, RpcError> {
            match self.pending.pop_front() {
                Some(Ok(Some(chunk))) => Ok(Some(Bytes::from(chunk))),
                Some(Ok(None)) | None => Ok(None),
                Some(Err(err)) => Err(err),
            }
        }
    }

    fn make_stream(pending: VecDeque<Result<Option<Vec<u8>>, RpcError>>) -> Arc<Mutex<MockStream>> {
        Arc::new(Mutex::new(MockStream::new(pending)))
    }

    fn encode_message(payload: &[u8]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(4 + payload.len());
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(payload);
        frame
    }

    #[tokio::test]
    async fn request_stream_with_initial_data_consumes_buffer_and_additional_chunks() {
        let message_one = b"hello".to_vec();
        let message_two = b"stream".to_vec();

        let mut initial = Vec::new();
        initial.extend_from_slice(&(message_one.len() as u32).to_le_bytes());
        initial.extend_from_slice(&message_one);
        initial.extend_from_slice(&(message_two.len() as u32).to_le_bytes());
        initial.extend_from_slice(&message_two[..3]);

        let mut pending = VecDeque::new();
        let mut tail = Vec::new();
        tail.extend_from_slice(&message_two[3..]);
        tail.extend_from_slice(&[0, 0, 0, 0]);
        pending.push_back(Ok(Some(tail)));

        let stream = make_stream(pending);
        let mut request_stream =
            RpcServer::create_request_stream_with_initial_data(stream, initial);

        let first = request_stream.next().await;
        assert_eq!(first.as_deref(), Some(message_one.as_slice()));

        let second = request_stream.next().await;
        assert_eq!(second.as_deref(), Some(message_two.as_slice()));

        assert!(request_stream.next().await.is_none());
    }

    #[tokio::test]
    async fn request_stream_with_initial_data_handles_receive_errors() {
        let mut pending = VecDeque::new();
        pending.push_back(Err(RpcError::StreamError("boom".into())));

        let stream = make_stream(pending);
        let mut request_stream =
            RpcServer::create_request_stream_with_initial_data(stream, Vec::new());

        assert!(request_stream.next().await.is_none());
    }

    #[tokio::test]
    async fn request_stream_handles_connection_close() {
        let mut pending = VecDeque::new();
        pending.push_back(Ok(None));

        let stream = make_stream(pending);
        let mut request_stream = RpcServer::create_request_stream(stream);

        assert!(request_stream.next().await.is_none());
    }

    #[tokio::test]
    async fn request_stream_exits_on_zero_length_marker() {
        let pending = VecDeque::from([Ok(Some(vec![0, 0, 0, 0]))]);
        let stream = make_stream(pending);
        let mut request_stream = RpcServer::create_request_stream(stream);

        assert!(request_stream.next().await.is_none());
    }

    #[tokio::test]
    async fn send_response_stream_writes_all_frames_and_end_marker() {
        let stream = make_stream(VecDeque::new());
        let frames = vec![Ok(b"one".to_vec()), Ok(b"two".to_vec())];

        RpcServer::send_response_stream(stream.clone(), Box::pin(stream::iter(frames))).await;

        let guard = stream.lock().await;
        assert_eq!(guard.sent.len(), 3);
        assert_eq!(guard.sent[0], encode_message(b"one"));
        assert_eq!(guard.sent[1], encode_message(b"two"));
        assert_eq!(guard.sent[2], vec![0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn send_response_stream_inserts_error_frame() {
        let stream = make_stream(VecDeque::new());
        let frames = vec![Ok(b"ok".to_vec()), Err(RpcError::StreamError("err".into()))];

        RpcServer::send_response_stream(stream.clone(), Box::pin(stream::iter(frames))).await;

        let guard = stream.lock().await;
        assert_eq!(guard.sent.len(), 3);
        assert_eq!(guard.sent[0], encode_message(b"ok"));
        assert_eq!(guard.sent[1], encode_message(b"Error processing request"));
        assert_eq!(guard.sent[2], vec![0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn send_response_stream_breaks_on_send_failure() {
        let stream = Arc::new(Mutex::new(
            MockStream::new(VecDeque::new()).with_send_error_after(0),
        ));
        let frames = vec![Ok(b"data".to_vec()), Ok(b"more".to_vec())];

        RpcServer::send_response_stream(stream.clone(), Box::pin(stream::iter(frames))).await;

        let guard = stream.lock().await;
        assert!(guard.sent.is_empty());
        assert_eq!(guard.send_calls, 0);
    }
}

#[cfg(test)]
mod server_register_tests {
    use super::{RpcConfig, RpcServer};
    use async_stream::stream;
    use futures::{stream as futures_stream, StreamExt};
    use std::time::Duration;

    pub(super) fn config() -> RpcConfig {
        RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("localhost")
            .with_keep_alive_interval(Duration::from_millis(50))
    }

    #[tokio::test]
    async fn register_streaming_inserts_handler_and_invokes_it() {
        let server = RpcServer::new(config());

        server
            .register_streaming("upper", |mut requests| async move {
                stream! {
                    while let Some(next) = requests.next().await {
                        yield Ok(next.iter().map(|b| b.to_ascii_uppercase()).collect());
                    }
                    yield Ok(b"done".to_vec());
                }
            })
            .await;

        let handlers = server.streaming_handlers.read().await;
        let handler = handlers.get("upper").expect("handler should be registered");

        let request_stream = Box::pin(futures_stream::iter(vec![b"abc".to_vec(), b"xyz".to_vec()]));
        let mut response_stream = handler(request_stream).await;

        let mut collected = Vec::new();
        while let Some(item) = response_stream.next().await {
            collected.push(item.expect("response should be Ok"));
        }

        assert_eq!(
            collected,
            vec![b"ABC".to_vec(), b"XYZ".to_vec(), b"done".to_vec()]
        );
    }

    #[tokio::test]
    async fn register_streaming_clones_handler_for_multiple_invocations() {
        let server = RpcServer::new(config());
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter = call_count.clone();

        server
            .register_streaming("count", move |requests| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    stream! {
                        let mut reqs = requests;
                        while let Some(item) = reqs.next().await {
                            yield Ok(item);
                        }
                    }
                }
            })
            .await;

        let handlers = server.streaming_handlers.read().await;
        let handler = handlers.get("count").unwrap();

        for _ in 0..2 {
            let request_stream = Box::pin(futures_stream::iter(Vec::<Vec<u8>>::new()));
            let mut response_stream = handler(request_stream).await;
            assert!(response_stream.next().await.is_none());
        }

        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}

#[cfg(test)]
mod client_streaming_helper_tests {
    use super::{QuicConnectionAdapter, QuicStreamAdapter, RpcClient, RpcError};
    use async_trait::async_trait;
    use bytes::Bytes;
    use futures::{stream, StreamExt};
    use std::{collections::VecDeque, sync::Arc, time::Duration};
    use tokio::sync::Mutex;

    struct MockStreamState {
        sent_frames: Vec<Vec<u8>>,
        send_plan: VecDeque<Result<(), RpcError>>,
        recv_plan: VecDeque<Result<Option<Vec<u8>>, RpcError>>,
    }

    impl MockStreamState {
        fn new() -> Self {
            Self {
                sent_frames: Vec::new(),
                send_plan: VecDeque::new(),
                recv_plan: VecDeque::new(),
            }
        }
    }

    #[derive(Clone)]
    struct MockStream {
        state: Arc<Mutex<MockStreamState>>,
    }

    #[async_trait]
    impl QuicStreamAdapter for MockStream {
        async fn send_bytes(&mut self, data: Bytes) -> Result<(), RpcError> {
            let mut state = self.state.lock().await;
            state.sent_frames.push(data.to_vec());
            match state.send_plan.pop_front() {
                Some(result) => result,
                None => Ok(()),
            }
        }

        async fn receive_bytes(&mut self) -> Result<Option<Bytes>, RpcError> {
            let mut state = self.state.lock().await;
            match state.recv_plan.pop_front() {
                Some(Ok(Some(chunk))) => Ok(Some(Bytes::from(chunk))),
                Some(Ok(None)) => Ok(None),
                Some(Err(err)) => Err(err),
                None => Ok(None),
            }
        }
    }

    struct MockConnection {
        state: Arc<Mutex<MockStreamState>>,
        open_error: Option<RpcError>,
    }

    impl MockConnection {
        fn new(state: Arc<Mutex<MockStreamState>>) -> Self {
            Self {
                state,
                open_error: None,
            }
        }

        fn with_error(error: RpcError) -> Self {
            Self {
                state: Arc::new(Mutex::new(MockStreamState::new())),
                open_error: Some(error),
            }
        }
    }

    #[async_trait]
    impl QuicConnectionAdapter for MockConnection {
        async fn open_bidirectional_stream(
            &mut self,
        ) -> Result<Box<dyn QuicStreamAdapter + Send>, RpcError> {
            if let Some(err) = self.open_error.take() {
                return Err(err);
            }

            Ok(Box::new(MockStream {
                state: self.state.clone(),
            }))
        }
    }

    fn encode_frame(payload: &[u8]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(4 + payload.len());
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(payload);
        frame
    }

    async fn wait_for_sent(state: &Arc<Mutex<MockStreamState>>, expected: usize) {
        for _ in 0..200 {
            {
                if state.lock().await.sent_frames.len() >= expected {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        // Final check to avoid missing race
        let len = state.lock().await.sent_frames.len();
        assert!(len >= expected, "expected {expected} frames but saw {len}");
    }

    #[tokio::test]
    async fn call_streaming_happy_path() {
        let state = Arc::new(Mutex::new(MockStreamState::new()));
        {
            let mut guard = state.lock().await;
            let mut response_chunk = encode_frame(b"resp-one");
            response_chunk.extend_from_slice(&encode_frame(b"resp-two"));
            response_chunk.extend_from_slice(&[0, 0, 0, 0]);
            guard.recv_plan.push_back(Ok(Some(response_chunk)));
            guard.recv_plan.push_back(Ok(None));
        }

        let connection = Box::new(MockConnection::new(state.clone()));
        let client = RpcClient::with_connection(connection);
        let requests = stream::iter(vec![b"req1".to_vec(), b"req2".to_vec()]);

        let response_stream = client
            .call_streaming("test_method", requests)
            .await
            .expect("call_streaming should succeed");
        let mut response_stream = Box::pin(response_stream);

        let mut responses = Vec::new();
        while let Some(item) = response_stream.next().await {
            responses.push(item.expect("response should be Ok"));
        }
        assert_eq!(responses, vec![b"resp-one".to_vec(), b"resp-two".to_vec()]);

        wait_for_sent(&state, 4).await;
        let sent_frames = { state.lock().await.sent_frames.clone() };
        assert_eq!(sent_frames.len(), 4);
        assert_eq!(sent_frames[0], encode_frame(b"test_method"));
        assert_eq!(sent_frames[1], encode_frame(b"req1"));
        assert_eq!(sent_frames[2], encode_frame(b"req2"));
        assert_eq!(sent_frames[3], vec![0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn call_streaming_method_send_error() {
        let state = Arc::new(Mutex::new(MockStreamState::new()));
        {
            let mut guard = state.lock().await;
            guard.send_plan.push_back(Err(RpcError::StreamError(
                "method send failure".to_string(),
            )));
        }

        let connection = Box::new(MockConnection::new(state.clone()));
        let client = RpcClient::with_connection(connection);

        let result = client
            .call_streaming("failing_method", stream::iter(Vec::<Vec<u8>>::new()))
            .await;

        assert!(matches!(result, Err(RpcError::StreamError(_))));
        let sent_frames = { state.lock().await.sent_frames.clone() };
        assert_eq!(sent_frames.len(), 1);
        assert_eq!(sent_frames[0], encode_frame(b"failing_method"));
    }

    #[tokio::test]
    async fn call_streaming_request_send_error_breaks_loop() {
        let state = Arc::new(Mutex::new(MockStreamState::new()));
        {
            let mut guard = state.lock().await;
            guard.send_plan.push_back(Ok(()));
            guard
                .send_plan
                .push_back(Err(RpcError::StreamError("loop send fail".into())));
            guard.send_plan.push_back(Ok(()));
            guard.recv_plan.push_back(Ok(Some(vec![0, 0, 0, 0])));
        }

        let connection = Box::new(MockConnection::new(state.clone()));
        let client = RpcClient::with_connection(connection);

        let response_stream = client
            .call_streaming("loop", stream::iter(vec![b"payload".to_vec()]))
            .await
            .expect("call_streaming should still return stream");
        let mut response_stream = Box::pin(response_stream);

        assert!(response_stream.next().await.is_none());

        wait_for_sent(&state, 3).await;
        let sent_frames = { state.lock().await.sent_frames.clone() };
        assert_eq!(sent_frames.len(), 3);
        assert_eq!(sent_frames[0], encode_frame(b"loop"));
        assert_eq!(sent_frames[1], encode_frame(b"payload"));
        assert_eq!(sent_frames[2], vec![0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn call_streaming_end_frame_send_error_is_suppressed() {
        let state = Arc::new(Mutex::new(MockStreamState::new()));
        {
            let mut guard = state.lock().await;
            guard.send_plan.push_back(Ok(()));
            guard.send_plan.push_back(Ok(()));
            guard
                .send_plan
                .push_back(Err(RpcError::StreamError("end frame".into())));
            guard.recv_plan.push_back(Ok(Some(vec![0, 0, 0, 0])));
        }

        let connection = Box::new(MockConnection::new(state.clone()));
        let client = RpcClient::with_connection(connection);

        let response_stream = client
            .call_streaming("end", stream::iter(vec![b"data".to_vec()]))
            .await
            .expect("call_streaming should succeed despite final send error");
        let mut response_stream = Box::pin(response_stream);
        assert!(response_stream.next().await.is_none());

        wait_for_sent(&state, 3).await;
        let sent_frames = { state.lock().await.sent_frames.clone() };
        assert_eq!(sent_frames.len(), 3);
        assert_eq!(sent_frames[0], encode_frame(b"end"));
        assert_eq!(sent_frames[1], encode_frame(b"data"));
        assert_eq!(sent_frames[2], vec![0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn call_streaming_zero_length_response_stops_stream() {
        let state = Arc::new(Mutex::new(MockStreamState::new()));
        {
            let mut guard = state.lock().await;
            guard.recv_plan.push_back(Ok(Some(vec![0, 0, 0, 0])));
        }

        let connection = Box::new(MockConnection::new(state));
        let client = RpcClient::with_connection(connection);

        let response_stream = client
            .call_streaming("stop", stream::iter(Vec::<Vec<u8>>::new()))
            .await
            .expect("call_streaming should succeed");
        let mut response_stream = Box::pin(response_stream);

        assert!(response_stream.next().await.is_none());
    }

    #[tokio::test]
    async fn call_streaming_incomplete_response_is_ignored() {
        let state = Arc::new(Mutex::new(MockStreamState::new()));
        {
            let mut guard = state.lock().await;
            let mut partial = encode_frame(b"incomplete");
            partial.truncate(6); // leave less than declared length
            guard.recv_plan.push_back(Ok(Some(partial)));
            guard.recv_plan.push_back(Ok(None));
        }

        let connection = Box::new(MockConnection::new(state));
        let client = RpcClient::with_connection(connection);

        let response_stream = client
            .call_streaming("ignore", stream::iter(Vec::<Vec<u8>>::new()))
            .await
            .expect("call_streaming should succeed");
        let mut response_stream = Box::pin(response_stream);
        assert!(response_stream.next().await.is_none());
    }

    #[tokio::test]
    async fn call_streaming_receive_error_stops_stream() {
        let state = Arc::new(Mutex::new(MockStreamState::new()));
        {
            let mut guard = state.lock().await;
            guard
                .recv_plan
                .push_back(Err(RpcError::StreamError("recv".into())));
        }

        let connection = Box::new(MockConnection::new(state));
        let client = RpcClient::with_connection(connection);

        let response_stream = client
            .call_streaming("recv_err", stream::iter(Vec::<Vec<u8>>::new()))
            .await
            .expect("call_streaming should succeed");
        let mut response_stream = Box::pin(response_stream);
        assert!(response_stream.next().await.is_none());
    }

    #[tokio::test]
    async fn call_streaming_open_failure_is_propagated() {
        let connection = Box::new(MockConnection::with_error(RpcError::StreamError(
            "open fail".into(),
        )));
        let client = RpcClient::with_connection(connection);

        let result = client
            .call_streaming("open", stream::iter(Vec::<Vec<u8>>::new()))
            .await;

        assert!(matches!(result, Err(RpcError::StreamError(_))));
    }
}

#[cfg(test)]
mod client_call_helper_tests {
    use super::{
        QuicConnectionAdapter, QuicStreamAdapter, RpcClient, RpcError, RpcRequest, RpcResponse,
    };
    use async_trait::async_trait;
    use bytes::Bytes;
    use std::{collections::VecDeque, sync::Arc, time::Duration};
    use tokio::sync::Mutex;

    pub(super) struct MockStreamState {
        pub sent: Vec<Vec<u8>>,
        pub send_plan: VecDeque<Result<(), RpcError>>,
        pub recv_plan: VecDeque<Result<Option<Vec<u8>>, RpcError>>,
    }

    impl MockStreamState {
        pub(super) fn new() -> Self {
            Self {
                sent: Vec::new(),
                send_plan: VecDeque::new(),
                recv_plan: VecDeque::new(),
            }
        }
    }

    #[derive(Clone)]
    pub(super) struct MockStream {
        state: Arc<Mutex<MockStreamState>>,
    }

    #[async_trait]
    impl QuicStreamAdapter for MockStream {
        async fn send_bytes(&mut self, data: Bytes) -> Result<(), RpcError> {
            let mut state = self.state.lock().await;
            state.sent.push(data.to_vec());
            match state.send_plan.pop_front() {
                Some(result) => result,
                None => Ok(()),
            }
        }

        async fn receive_bytes(&mut self) -> Result<Option<Bytes>, RpcError> {
            let mut state = self.state.lock().await;
            match state.recv_plan.pop_front() {
                Some(Ok(Some(chunk))) => Ok(Some(Bytes::from(chunk))),
                Some(Ok(None)) => Ok(None),
                Some(Err(err)) => Err(err),
                None => Ok(None),
            }
        }
    }

    pub(super) struct MockConnection {
        streams: VecDeque<Arc<Mutex<MockStreamState>>>,
        stream_error: Option<RpcError>,
    }

    impl MockConnection {
        pub(super) fn with_state(state: Arc<Mutex<MockStreamState>>) -> Self {
            let mut streams = VecDeque::new();
            streams.push_back(state);
            Self {
                streams,
                stream_error: None,
            }
        }

        pub(super) fn with_states(states: Vec<Arc<Mutex<MockStreamState>>>) -> Self {
            Self {
                streams: states.into(),
                stream_error: None,
            }
        }

        pub(super) fn with_error(error: RpcError) -> Self {
            Self {
                streams: VecDeque::new(),
                stream_error: Some(error),
            }
        }
    }

    #[async_trait]
    impl QuicConnectionAdapter for MockConnection {
        async fn open_bidirectional_stream(
            &mut self,
        ) -> Result<Box<dyn QuicStreamAdapter + Send>, RpcError> {
            if let Some(err) = self.stream_error.take() {
                return Err(err);
            }
            let state = self
                .streams
                .pop_front()
                .unwrap_or_else(|| Arc::new(Mutex::new(MockStreamState::new())));
            Ok(Box::new(MockStream { state }))
        }
    }

    pub(super) fn make_client(state: Arc<Mutex<MockStreamState>>) -> RpcClient {
        RpcClient::with_connection(Box::new(MockConnection::with_state(state)))
    }

    pub(super) fn make_client_with_connection(connection: MockConnection) -> RpcClient {
        RpcClient::with_connection(Box::new(connection))
    }

    pub(super) fn encode_response(response: &RpcResponse) -> Vec<u8> {
        bincode::serialize(response).expect("serialize response")
    }

    pub(super) async fn wait_for_sent(state: &Arc<Mutex<MockStreamState>>, expected: usize) {
        for _ in 0..200 {
            if state.lock().await.sent.len() >= expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let len = state.lock().await.sent.len();
        panic!("expected {} frames, saw {}", expected, len);
    }

    #[tokio::test]
    async fn call_returns_success_response() {
        let state = Arc::new(Mutex::new(MockStreamState::new()));
        {
            let mut guard = state.lock().await;
            let response = RpcResponse::from_result(1, Ok(b"pong".to_vec()));
            guard
                .recv_plan
                .push_back(Ok(Some(encode_response(&response))));
        }

        let client = make_client(state.clone());
        let response = client.call("ping", b"data".to_vec()).await.unwrap();
        assert_eq!(response, b"pong".to_vec());

        wait_for_sent(&state, 1).await;
        let sent = state.lock().await.sent.clone();
        assert_eq!(sent.len(), 1);
        let request: RpcRequest = bincode::deserialize(&sent[0]).unwrap();
        assert_eq!(request.method(), "ping");
    }

    #[tokio::test]
    async fn call_propagates_error_response() {
        let state = Arc::new(Mutex::new(MockStreamState::new()));
        {
            let mut guard = state.lock().await;
            let response = RpcResponse::from_result(1, Err(RpcError::StreamError("nope".into())));
            guard
                .recv_plan
                .push_back(Ok(Some(encode_response(&response))));
        }

        let client = make_client(state);
        let err = client.call("fails", Vec::new()).await.unwrap_err();
        match err {
            RpcError::StreamError(msg) => assert!(msg.contains("nope")),
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[tokio::test]
    async fn call_detects_connection_close_before_response() {
        let state = Arc::new(Mutex::new(MockStreamState::new()));
        {
            let mut guard = state.lock().await;
            guard.recv_plan.push_back(Ok(None));
        }

        let client = make_client(state);
        let err = client.call("timeout", Vec::new()).await.unwrap_err();
        assert!(matches!(err, RpcError::ConnectionError(_)));
    }

    #[tokio::test]
    async fn call_open_stream_failure_is_propagated() {
        let client = RpcClient::with_connection(Box::new(MockConnection::with_error(
            RpcError::StreamError("open".into()),
        )));
        let err = client.call("method", Vec::new()).await.unwrap_err();
        assert!(matches!(err, RpcError::StreamError(_)));
    }
}

#[cfg(test)]
mod server_start_helper_tests {
    use super::{
        QuicServerAdapter, QuicServerConnectionAdapter, QuicStreamAdapter, RpcConfig, RpcError,
        RpcServer,
    };
    use async_stream::stream;
    use async_trait::async_trait;
    use bytes::Bytes;
    use futures::StreamExt;
    use std::{collections::VecDeque, sync::Arc, time::Duration};
    use tokio::sync::Mutex;

    pub(super) struct MockStreamState {
        pub read_plan: VecDeque<Result<Option<Vec<u8>>, RpcError>>,
        pub writes: Vec<Vec<u8>>,
    }

    impl MockStreamState {
        pub(super) fn new() -> Self {
            Self {
                read_plan: VecDeque::new(),
                writes: Vec::new(),
            }
        }
    }

    #[derive(Clone)]
    pub(super) struct MockStream {
        pub state: Arc<Mutex<MockStreamState>>,
    }

    #[async_trait]
    impl QuicStreamAdapter for MockStream {
        async fn send_bytes(&mut self, data: Bytes) -> Result<(), RpcError> {
            self.state.lock().await.writes.push(data.to_vec());
            Ok(())
        }

        async fn receive_bytes(&mut self) -> Result<Option<Bytes>, RpcError> {
            let mut state = self.state.lock().await;
            match state.read_plan.pop_front() {
                Some(Ok(Some(chunk))) => Ok(Some(Bytes::from(chunk))),
                Some(Ok(None)) => Ok(None),
                Some(Err(err)) => Err(err),
                None => Ok(None),
            }
        }
    }

    pub(super) struct MockServerConnection {
        streams: VecDeque<Result<Option<MockStream>, RpcError>>,
    }

    impl MockServerConnection {
        pub(super) fn new() -> Self {
            Self {
                streams: VecDeque::new(),
            }
        }

        pub(super) fn push_stream(&mut self, stream: MockStream) {
            self.streams.push_back(Ok(Some(stream)));
        }

        pub(super) fn finish(&mut self) {
            self.streams.push_back(Ok(None));
        }
    }

    #[async_trait]
    impl QuicServerConnectionAdapter for MockServerConnection {
        async fn accept_bidirectional_stream(
            &mut self,
        ) -> Result<Option<Box<dyn QuicStreamAdapter + Send>>, RpcError> {
            match self.streams.pop_front() {
                Some(Ok(Some(stream))) => Ok(Some(Box::new(stream))),
                Some(Ok(None)) => Ok(None),
                Some(Err(err)) => Err(err),
                None => Ok(None),
            }
        }
    }

    pub(super) struct MockServerAdapter {
        connections: VecDeque<Box<dyn QuicServerConnectionAdapter>>,
    }

    impl MockServerAdapter {
        pub(super) fn new(connections: Vec<Box<dyn QuicServerConnectionAdapter>>) -> Self {
            Self {
                connections: connections.into(),
            }
        }
    }

    #[async_trait]
    impl QuicServerAdapter for MockServerAdapter {
        async fn accept(&mut self) -> Option<Box<dyn QuicServerConnectionAdapter>> {
            self.connections.pop_front()
        }
    }

    pub(super) fn test_config() -> RpcConfig {
        RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("localhost")
            .with_keep_alive_interval(Duration::from_millis(50))
    }

    pub(super) async fn wait_for_writes(state: &Arc<Mutex<MockStreamState>>, expected: usize) {
        for _ in 0..100 {
            if state.lock().await.writes.len() >= expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let len = state.lock().await.writes.len();
        panic!("expected {} writes, saw {}", expected, len);
    }

    #[tokio::test]
    async fn start_with_adapter_processes_streaming_request() {
        let mut server = RpcServer::new(test_config());
        server
            .register_streaming("stream_method", |mut requests| async move {
                stream! {
                    while let Some(data) = requests.next().await {
                        let mut upper = data.clone();
                        upper.make_ascii_uppercase();
                        yield Ok(upper);
                    }
                    yield Ok(b"DONE".to_vec());
                }
            })
            .await;

        let stream_state = Arc::new(Mutex::new(MockStreamState::new()));
        {
            let mut guard = stream_state.lock().await;
            let method = b"stream_method";
            let mut chunk = Vec::new();
            chunk.extend_from_slice(&(method.len() as u32).to_le_bytes());
            chunk.extend_from_slice(method);
            chunk.extend_from_slice(&(3u32).to_le_bytes());
            chunk.extend_from_slice(b"one");
            chunk.extend_from_slice(&(0u32).to_le_bytes());
            guard.read_plan.push_back(Ok(Some(chunk)));
            guard.read_plan.push_back(Ok(None));
        }

        let stream = MockStream {
            state: stream_state.clone(),
        };
        let mut connection = MockServerConnection::new();
        connection.push_stream(stream.clone());
        connection.finish();

        let mut adapter = MockServerAdapter::new(vec![Box::new(connection)]);
        server.start_with_adapter(&mut adapter).await.unwrap();

        wait_for_writes(&stream_state, 3).await;
        let writes = stream_state.lock().await.writes.clone();
        assert_eq!(writes.len(), 3);
        assert_eq!(writes[0][..4], (3u32).to_le_bytes());
        assert_eq!(&writes[0][4..], b"ONE");
        assert_eq!(writes[1][..4], (4u32).to_le_bytes());
        assert_eq!(&writes[1][4..], b"DONE");
        assert_eq!(writes[2], vec![0, 0, 0, 0]);
    }
}

#[cfg(test)]
mod doc_examples_tests {
    use std::time::Duration;
    use super::{
        client_call_helper_tests::{
            encode_response as encode_rpc_response, make_client, make_client_with_connection,
            wait_for_sent as wait_for_sent_frames, MockConnection as CallMockConnection,
            MockStreamState as CallMockStreamState,
        },
        server_register_tests::config as server_test_config,
        server_start_helper_tests::{
            wait_for_writes as wait_for_server_writes, MockServerAdapter, MockServerConnection,
            MockStream as ServerMockStream, MockStreamState as ServerMockStreamState,
        },
        RpcRequest, RpcResponse, RpcServer,
    };
    use async_stream::stream;
    use futures::{stream as futures_stream, StreamExt};
    use std::sync::Arc;
    use tokio::join;
    use tokio::sync::Mutex;

    fn encode_frame(payload: &[u8]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(4 + payload.len());
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(payload);
        frame
    }

    #[tokio::test]
    async fn rpc_server_register_doc_example() {
        let server = RpcServer::new(server_test_config());
        server
            .register("add", |params| async move { Ok(params) })
            .await;

        let handlers = server.handlers.read().await;
        let handler = handlers.get("add").expect("handler should be registered");
        let response = handler(vec![1, 2, 3])
            .await
            .expect("handler should succeed");
        assert_eq!(response, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn rpc_server_register_streaming_doc_example() {
        let server = RpcServer::new(server_test_config());

        server
            .register_streaming("echo_stream", |mut request_stream| async move {
                stream! {
                    while let Some(data) = request_stream.next().await {
                        yield Ok(data);
                    }
                }
            })
            .await;

        let handlers = server.streaming_handlers.read().await;
        let handler = handlers
            .get("echo_stream")
            .expect("streaming handler should be registered");

        let request_stream = Box::pin(futures_stream::iter(vec![b"one".to_vec(), b"two".to_vec()]));
        let mut responses = handler(request_stream).await;

        let mut collected = Vec::new();
        while let Some(item) = responses.next().await {
            collected.push(item.expect("response should be Ok"));
        }

        assert_eq!(collected, vec![b"one".to_vec(), b"two".to_vec()]);
    }

    #[tokio::test]
    async fn rpc_client_call_doc_example() {
        let state = Arc::new(Mutex::new(CallMockStreamState::new()));
        {
            let mut guard = state.lock().await;
            let response = RpcResponse::from_result(1, Ok(b"Hello, Server!".to_vec()));
            guard
                .recv_plan
                .push_back(Ok(Some(encode_rpc_response(&response))));
        }

        let client = make_client(state.clone());
        let response = client
            .call("echo", bincode::serialize(&"Hello, Server!").unwrap())
            .await
            .unwrap();
        assert_eq!(response, b"Hello, Server!".to_vec());

        wait_for_sent_frames(&state, 1).await;
        let sent_requests = state.lock().await.sent.clone();
        let req: RpcRequest = bincode::deserialize(&sent_requests[0]).unwrap();
        assert_eq!(req.method(), "echo");
    }

    #[tokio::test]
    async fn rpc_client_call_concurrent_doc_example() {
        let state_one = Arc::new(Mutex::new(CallMockStreamState::new()));
        {
            let mut guard = state_one.lock().await;
            let resp = RpcResponse::from_result(1, Ok(b"result1".to_vec()));
            guard
                .recv_plan
                .push_back(Ok(Some(encode_rpc_response(&resp))));
        }

        let state_two = Arc::new(Mutex::new(CallMockStreamState::new()));
        {
            let mut guard = state_two.lock().await;
            let resp = RpcResponse::from_result(2, Ok(b"result2".to_vec()));
            guard
                .recv_plan
                .push_back(Ok(Some(encode_rpc_response(&resp))));
        }

        let connection =
            CallMockConnection::with_states(vec![state_one.clone(), state_two.clone()]);
        let client = Arc::new(make_client_with_connection(connection));

        let (res_left, res_right) = join!(
            {
                let client = client.clone();
                async move { client.call("method1", vec![]).await.unwrap() }
            },
            {
                let client = client.clone();
                async move { client.call("method2", vec![]).await.unwrap() }
            }
        );

        assert_eq!(res_left, b"result1".to_vec());
        assert_eq!(res_right, b"result2".to_vec());

        wait_for_sent_frames(&state_one, 1).await;
        wait_for_sent_frames(&state_two, 1).await;

        let sent_one = state_one.lock().await.sent.clone();
        let req_one: RpcRequest = bincode::deserialize(&sent_one[0]).unwrap();
        assert_eq!(req_one.method(), "method1");

        let sent_two = state_two.lock().await.sent.clone();
        let req_two: RpcRequest = bincode::deserialize(&sent_two[0]).unwrap();
        assert_eq!(req_two.method(), "method2");
    }

    #[tokio::test]
    async fn rpc_client_call_streaming_doc_example() {
        let state = Arc::new(Mutex::new(CallMockStreamState::new()));
        {
            let mut guard = state.lock().await;
            let mut chunk = Vec::new();
            chunk.extend_from_slice(&encode_frame(b"response1"));
            chunk.extend_from_slice(&encode_frame(b"response2"));
            chunk.extend_from_slice(&[0, 0, 0, 0]);
            guard.recv_plan.push_back(Ok(Some(chunk)));
        }

        let client = make_client(state.clone());
        let response_stream = client
            .call_streaming(
                "echo_stream",
                futures_stream::iter(vec![b"request1".to_vec(), b"request2".to_vec()]),
            )
            .await
            .unwrap();
        let mut response_stream = Box::pin(response_stream);

        let mut collected = Vec::new();
        while let Some(item) = response_stream.next().await {
            collected.push(item.unwrap());
        }
        assert_eq!(
            collected,
            vec![b"response1".to_vec(), b"response2".to_vec()]
        );

        wait_for_sent_frames(&state, 4).await;
        let writes = state.lock().await.sent.clone();
        assert_eq!(writes.len(), 4);
        assert_eq!(writes[0], encode_frame(b"echo_stream"));
        assert_eq!(writes[1], encode_frame(b"request1"));
        assert_eq!(writes[2], encode_frame(b"request2"));
        assert_eq!(writes[3], vec![0, 0, 0, 0]);
    }

    #[tokio::test]
    async fn rpc_client_call_server_streaming_doc_example() {
        let state = Arc::new(Mutex::new(CallMockStreamState::new()));
        {
            let mut guard = state.lock().await;
            let mut chunk = Vec::new();
            chunk.extend_from_slice(&encode_frame(b"entry1"));
            chunk.extend_from_slice(&encode_frame(b"entry2"));
            chunk.extend_from_slice(&[0, 0, 0, 0]);
            guard.recv_plan.push_back(Ok(Some(chunk)));
        }

        let client = make_client(state);
        let response_stream = client.call_server_streaming("list", vec![]).await.unwrap();
        let mut response_stream = Box::pin(response_stream);

        let mut collected = Vec::new();
        while let Some(item) = response_stream.next().await {
            collected.push(item.unwrap());
        }
        assert_eq!(collected, vec![b"entry1".to_vec(), b"entry2".to_vec()]);
    }

    #[tokio::test]
    async fn rpc_client_call_client_streaming_doc_example() {
        let state = Arc::new(Mutex::new(CallMockStreamState::new()));
        {
            let mut guard = state.lock().await;
            let mut chunk = Vec::new();
            chunk.extend_from_slice(&encode_frame(b"aggregate"));
            chunk.extend_from_slice(&[0, 0, 0, 0]);
            guard.recv_plan.push_back(Ok(Some(chunk)));
        }

        let client = make_client(state);
        let result = client
            .call_client_streaming(
                "aggregate",
                futures_stream::iter(vec![b"part1".to_vec(), b"part2".to_vec()]),
            )
            .await
            .unwrap();

        assert_eq!(result, b"aggregate".to_vec());
    }

    #[tokio::test]
    async fn rpc_server_start_doc_example_via_adapter() {
        let mut server = RpcServer::new(server_test_config());
        server
            .register_streaming("stream_method", |mut reqs| async move {
                stream! {
                    while let Some(data) = reqs.next().await {
                        yield Ok(data);
                    }
                }
            })
            .await;

        let stream_state = Arc::new(Mutex::new(ServerMockStreamState::new()));
        {
            let mut guard = stream_state.lock().await;
            let mut request_buffer = Vec::new();
            request_buffer.extend_from_slice(&encode_frame(b"stream_method"));
            request_buffer.extend_from_slice(&encode_frame(b"payload"));
            request_buffer.extend_from_slice(&[0, 0, 0, 0]);
            guard.read_plan.push_back(Ok(Some(request_buffer)));
            guard.read_plan.push_back(Ok(None));
        }

        let stream = ServerMockStream {
            state: stream_state.clone(),
        };
        let mut connection = MockServerConnection::new();
        connection.push_stream(stream);
        connection.finish();

        let mut adapter = MockServerAdapter::new(vec![Box::new(connection)]);
        server.start_with_adapter(&mut adapter).await.unwrap();

        wait_for_server_writes(&stream_state, 2).await;
        let writes = stream_state.lock().await.writes.clone();
        assert_eq!(writes.last().unwrap(), &vec![0, 0, 0, 0]);
    }
}

// ==========================
//          TESTS
// ==========================
#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::SocketAddr, str::FromStr};
    use tokio::{spawn, time::sleep};

    // For your local tests, adjust cert/key paths if needed:
    fn test_config() -> RpcConfig {
        RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
            .with_key_path("certs/test_key.pem")
            .with_server_name("localhost")
            .with_keep_alive_interval(Duration::from_secs(30))
    }

    async fn start_test_server(
        maybe_server: Option<RpcServer>,
    ) -> Result<(SocketAddr, tokio::task::JoinHandle<Result<(), RpcError>>), RpcError> {
        let server = if let Some(s) = maybe_server {
            s
        } else {
            let s = RpcServer::new(test_config());
            // A simple "echo" handler
            s.register("echo", |params| async move {
                Ok(params) // just echo
            })
            .await;
            s
        };

        let key_path = server
            .config
            .key_path
            .as_ref()
            .ok_or_else(|| RpcError::ConfigError("No key path".into()))?;

        let mut quic_server = s2n_quic::Server::builder()
            .with_tls((server.config.cert_path.as_path(), key_path.as_path()))
            .map_err(|e| RpcError::TlsError(format!("{:?}", e)))?
            .with_io(server.config.bind_address.as_str())
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?
            .start()
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?;

        let local_addr = quic_server
            .local_addr()
            .map_err(|_| RpcError::ConfigError("Could not retrieve local addr".into()))?;

        let handlers = server.handlers.clone();
        let handle = spawn(async move {
            while let Some(mut connection) = quic_server.accept().await {
                let handlers = handlers.clone();
                tokio::spawn(async move {
                    while let Ok(Some(mut stream)) = connection.accept_bidirectional_stream().await
                    {
                        let handlers = handlers.clone();
                        tokio::spawn(async move {
                            let mut request_data = Vec::with_capacity(8192);
                            while let Ok(Some(data)) = stream.receive().await {
                                request_data.extend_from_slice(&data);
                                if let Ok(request) =
                                    bincode::deserialize::<RpcRequest>(&request_data)
                                {
                                    let handlers = handlers.read().await;
                                    let response = match handlers.get(request.method()) {
                                        Some(handler) => {
                                            let result = handler(request.params().to_vec()).await;
                                            RpcResponse::from_result(request.id(), result)
                                        }
                                        None => RpcResponse::new(
                                            request.id(),
                                            None,
                                            Some(format!("Unknown method: {}", request.method())),
                                        ),
                                    };
                                    if let Ok(resp_data) = bincode::serialize(&response) {
                                        let _ = stream.send(resp_data.into()).await;
                                    }
                                    break;
                                }
                            }
                        });
                    }
                });
            }
            Ok(())
        });

        Ok((local_addr, handle))
    }

    // -----------------------------------
    // tests
    // -----------------------------------
    #[tokio::test]
    async fn test_config_builder() {
        let config = RpcConfig::new("certs/cert.pem", "127.0.0.1:8080")
            .with_key_path("certs/key.pem")
            .with_server_name("mytest.server")
            .with_keep_alive_interval(Duration::from_secs(60));

        assert_eq!(config.cert_path, PathBuf::from("certs/cert.pem"));
        assert_eq!(config.key_path, Some(PathBuf::from("certs/key.pem")));
        assert_eq!(config.server_name, "mytest.server");
        assert_eq!(config.bind_address, "127.0.0.1:8080");
        assert_eq!(config.keep_alive_interval, Some(Duration::from_secs(60)));
    }

    #[tokio::test]
    async fn test_register_handler() {
        let server = RpcServer::new(test_config());
        server
            .register("test", |params| async move {
                Ok(params) // echo
            })
            .await;

        let handlers = server.handlers.read().await;
        assert!(handlers.contains_key("test"));
    }

    #[tokio::test]
    async fn test_unknown_method() {
        let server = RpcServer::new(test_config());
        // no method registered => unknown

        // We'll do a small direct test function:
        async fn handle_request_direct(
            server: &RpcServer,
            req_data: Vec<u8>,
        ) -> Result<Vec<u8>, RpcError> {
            let req: RpcRequest = bincode::deserialize(&req_data)?;
            let handlers = server.handlers.read().await;
            let h = handlers
                .get(req.method())
                .ok_or_else(|| RpcError::UnknownMethod(req.method().to_string()))?;

            let result = h(req.params().to_vec()).await;
            let resp = RpcResponse::from_result(req.id(), result);
            Ok(bincode::serialize(&resp)?)
        }

        let req = RpcRequest::new(1, "unknown".into(), vec![]);
        let data = bincode::serialize(&req).unwrap();

        let res = handle_request_direct(&server, data).await;
        match res {
            Err(RpcError::UnknownMethod(m)) => assert_eq!(m, "unknown"),
            other => panic!("Expected UnknownMethod, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_handle_request() {
        let server = RpcServer::new(test_config());
        server
            .register("echo", |params| async move {
                Ok(params) // echo
            })
            .await;

        async fn handle_request_direct(
            server: &RpcServer,
            req_data: Vec<u8>,
        ) -> Result<Vec<u8>, RpcError> {
            let req: RpcRequest = bincode::deserialize(&req_data)?;
            let handlers = server.handlers.read().await;
            let h = handlers
                .get(req.method())
                .ok_or_else(|| RpcError::UnknownMethod(req.method().to_string()))?;

            let result = h(req.params().to_vec()).await;
            let resp = RpcResponse::from_result(req.id(), result);
            Ok(bincode::serialize(&resp)?)
        }

        let req = RpcRequest::new(42, "echo".into(), b"hello".to_vec());
        let data = bincode::serialize(&req).unwrap();
        let res_data = handle_request_direct(&server, data).await.unwrap();
        let resp: RpcResponse = bincode::deserialize(&res_data).unwrap();

        assert_eq!(resp.id(), 42);
        assert_eq!(resp.result().unwrap(), b"hello");
    }

    #[tokio::test]
    async fn test_client_connection() -> Result<(), RpcError> {
        let (addr, _jh) = start_test_server(None).await?;
        let client = RpcClient::connect(addr, test_config()).await?;

        assert_eq!(client.next_id.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_call_timeout() -> Result<(), RpcError> {
        let server = RpcServer::new(test_config());
        server
            .register("slow_method", |_params| async {
                // Sleep 3s so the 2s test timeout reliably hits
                sleep(Duration::from_secs(3)).await;
                Ok(b"done".to_vec())
            })
            .await;

        let (addr, _jh) = start_test_server(Some(server)).await?;
        let client = RpcClient::connect(addr, test_config()).await?;

        // Expect a 2s timeout
        let result = client.call("slow_method", vec![]).await;
        assert!(matches!(result, Err(RpcError::Timeout)));

        Ok(())
    }

    #[tokio::test]
    async fn test_request_ids() -> Result<(), RpcError> {
        let (addr, _jh) = start_test_server(None).await?;
        let client = RpcClient::connect(addr, test_config()).await?;

        let id1 = client.next_id.fetch_add(1, Ordering::SeqCst);
        let id2 = client.next_id.fetch_add(1, Ordering::SeqCst);
        let id3 = client.next_id.fetch_add(1, Ordering::SeqCst);

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_calls() -> Result<(), RpcError> {
        let server = RpcServer::new(test_config());
        server
            .register("test_method", |_params| async {
                // Sleep longer than the 2s default test timeout
                sleep(Duration::from_secs(3)).await;
                Ok(vec![1, 2, 3])
            })
            .await;

        let (addr, _jh) = start_test_server(Some(server)).await?;
        let client = Arc::new(RpcClient::connect(addr, test_config()).await?);

        let mut tasks = vec![];
        for _ in 0..5 {
            let c = client.clone();
            tasks.push(tokio::spawn(
                async move { c.call("test_method", vec![]).await },
            ));
        }

        for t in tasks {
            let res = t.await.unwrap();
            assert!(matches!(res, Err(RpcError::Timeout)));
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_connection_error() -> Result<(), RpcError> {
        // Connect to something that doesn't exist
        let addr = SocketAddr::from_str("127.0.0.1:9999").unwrap();
        let res = RpcClient::connect(addr, test_config()).await;
        assert!(matches!(res, Err(RpcError::ConnectionError(_))));
        Ok(())
    }

    #[tokio::test]
    async fn test_server_bind_error_missing_key() {
        let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0");
        // No key path configured
        let mut server = RpcServer::new(config);
        let result = server.bind();
        assert!(matches!(result, Err(RpcError::ConfigError(_))));
    }

    #[tokio::test]
    async fn test_server_socket_addr() -> Result<(), RpcError> {
        let mut server = RpcServer::new(test_config());
        assert_eq!(server.socket_addr, None);

        let _quic_server = server.bind()?;
        assert!(server.socket_addr.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_rpc_request_accessors() {
        let params = vec![1, 2, 3, 4, 5];
        let request = RpcRequest::new(42, "test_method".to_string(), params.clone());

        assert_eq!(request.id(), 42);
        assert_eq!(request.method(), "test_method");
        assert_eq!(request.params(), &params);
    }

    #[tokio::test]
    async fn test_rpc_response_accessors() {
        let result_data = vec![10, 20, 30];
        let error_msg = "test error".to_string();

        // Test success response
        let success_response = RpcResponse::new(123, Some(result_data.clone()), None);
        assert_eq!(success_response.id(), 123);
        assert_eq!(success_response.result(), Some(&result_data));
        assert_eq!(success_response.error(), None);

        // Test error response
        let error_response = RpcResponse::new(456, None, Some(error_msg.clone()));
        assert_eq!(error_response.id(), 456);
        assert_eq!(error_response.result(), None);
        assert_eq!(error_response.error(), Some(&error_msg));
    }

    #[tokio::test]
    async fn test_rpc_response_from_result() {
        // Test Ok result
        let ok_result: Result<Vec<u8>, RpcError> = Ok(vec![1, 2, 3]);
        let response = RpcResponse::from_result(100, ok_result);
        assert_eq!(response.id(), 100);
        assert_eq!(response.result(), Some(&vec![1, 2, 3]));
        assert_eq!(response.error(), None);

        // Test Err result
        let err_result: Result<Vec<u8>, RpcError> = Err(RpcError::Timeout);
        let response = RpcResponse::from_result(200, err_result);
        assert_eq!(response.id(), 200);
        assert_eq!(response.result(), None);
        assert!(response.error().is_some());
        assert!(response.error().unwrap().contains("timeout"));
    }

    #[tokio::test]
    async fn test_config_cloning() {
        let original = RpcConfig::new("test.pem", "127.0.0.1:8080")
            .with_key_path("key.pem")
            .with_server_name("test.server")
            .with_keep_alive_interval(Duration::from_secs(60));

        let cloned = original.clone();

        assert_eq!(original.cert_path, cloned.cert_path);
        assert_eq!(original.key_path, cloned.key_path);
        assert_eq!(original.bind_address, cloned.bind_address);
        assert_eq!(original.server_name, cloned.server_name);
        assert_eq!(original.keep_alive_interval, cloned.keep_alive_interval);
    }

    #[tokio::test]
    async fn test_error_display_formats() {
        let errors = vec![
            RpcError::ConnectionError("connection failed".to_string()),
            RpcError::StreamError("stream closed".to_string()),
            RpcError::TlsError("handshake failed".to_string()),
            RpcError::Timeout,
            RpcError::UnknownMethod("missing_method".to_string()),
            RpcError::ConfigError("bad config".to_string()),
        ];

        for error in errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty());
            println!("Error: {}", error_string);
        }
    }

    #[tokio::test]
    async fn test_large_payload_serialization() {
        let large_data = vec![0xAA; 100_000]; // 100KB
        let request = RpcRequest::new(999, "large_test".to_string(), large_data.clone());

        // Should serialize and deserialize successfully
        let serialized = bincode::serialize(&request).unwrap();
        let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.id(), 999);
        assert_eq!(deserialized.method(), "large_test");
        assert_eq!(deserialized.params(), &large_data);
    }

    #[tokio::test]
    async fn test_empty_method_and_params() {
        let request = RpcRequest::new(0, "".to_string(), vec![]);
        assert_eq!(request.method(), "");
        assert!(request.params().is_empty());

        // Should be serializable
        let serialized = bincode::serialize(&request).unwrap();
        let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.method(), "");
        assert!(deserialized.params().is_empty());
    }

    #[tokio::test]
    async fn test_multiple_handler_registration() -> Result<(), RpcError> {
        let server = RpcServer::new(test_config());

        // Register multiple handlers
        server
            .register("method1", |_| async move { Ok(b"response1".to_vec()) })
            .await;
        server
            .register("method2", |_| async move { Ok(b"response2".to_vec()) })
            .await;
        server
            .register("method3", |_| async move { Ok(b"response3".to_vec()) })
            .await;

        let handlers = server.handlers.read().await;
        assert_eq!(handlers.len(), 3);
        assert!(handlers.contains_key("method1"));
        assert!(handlers.contains_key("method2"));
        assert!(handlers.contains_key("method3"));

        Ok(())
    }

    #[tokio::test]
    async fn test_handler_overwrite() -> Result<(), RpcError> {
        let server = RpcServer::new(test_config());

        // Register handler
        server
            .register("test", |_| async move { Ok(b"first".to_vec()) })
            .await;

        let handlers = server.handlers.read().await;
        assert_eq!(handlers.len(), 1);
        drop(handlers);

        // Overwrite with new handler
        server
            .register("test", |_| async move { Ok(b"second".to_vec()) })
            .await;

        let handlers = server.handlers.read().await;
        assert_eq!(handlers.len(), 1); // Still only one handler
        assert!(handlers.contains_key("test"));

        Ok(())
    }

    #[tokio::test]
    async fn test_client_id_generation() -> Result<(), RpcError> {
        let (addr, _jh) = start_test_server(None).await?;
        let client = RpcClient::connect(addr, test_config()).await?;

        // Initial ID should be 1
        assert_eq!(client.next_id.load(Ordering::SeqCst), 1);

        // After making a call, ID should be incremented
        let _response = client.call("echo", vec![1, 2, 3]).await?;
        assert_eq!(client.next_id.load(Ordering::SeqCst), 2);

        // Multiple calls should increment ID
        let _response = client.call("echo", vec![]).await?;
        let _response = client.call("echo", vec![]).await?;
        assert_eq!(client.next_id.load(Ordering::SeqCst), 4);

        Ok(())
    }

    #[tokio::test]
    async fn test_default_timeout_constant() {
        // Test that the timeout constant is properly defined
        #[cfg(test)]
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(2));

        #[cfg(not(test))]
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_config_pathbuf_compatibility() {
        use std::path::PathBuf;

        // Test with &str
        let config1 = RpcConfig::new("cert.pem", "127.0.0.1:0");
        assert_eq!(config1.cert_path, PathBuf::from("cert.pem"));

        // Test with String
        let config2 = RpcConfig::new("cert.pem".to_string(), "127.0.0.1:0".to_string());
        assert_eq!(config2.cert_path, PathBuf::from("cert.pem"));

        // Test with PathBuf
        let config3 = RpcConfig::new(PathBuf::from("cert.pem"), "127.0.0.1:0");
        assert_eq!(config3.cert_path, PathBuf::from("cert.pem"));
    }

    #[test]
    fn test_config_builder_doctest() {
        use std::path::PathBuf;
        use std::time::Duration;

        let config = RpcConfig::new("cert.pem", "127.0.0.1:8080")
            .with_key_path("key.pem")
            .with_server_name("example.com")
            .with_keep_alive_interval(Duration::from_secs(60));

        assert_eq!(config.cert_path, PathBuf::from("cert.pem"));
        assert_eq!(config.bind_address, "127.0.0.1:8080");
        assert_eq!(config.server_name, "example.com");
    }

    #[test]
    fn test_request_creation_doctest() {
        let request = RpcRequest::new(123, "test_method".to_string(), vec![1, 2, 3]);

        assert_eq!(request.id(), 123);
        assert_eq!(request.method(), "test_method");
        assert_eq!(request.params(), &[1, 2, 3]);
    }

    #[test]
    fn test_response_creation_doctest() {
        // Success response
        let success = RpcResponse::new(1, Some(vec![42]), None);
        assert_eq!(success.id(), 1);
        assert_eq!(success.result(), Some(&vec![42]));
        assert!(success.error().is_none());

        // Error response
        let error_resp = RpcResponse::new(2, None, Some("Error occurred".to_string()));
        assert_eq!(error_resp.id(), 2);
        assert!(error_resp.result().is_none());
        assert_eq!(error_resp.error(), Some(&"Error occurred".to_string()));
    }

    #[test]
    fn test_error_display_doctest() {
        let errors = vec![
            RpcError::ConnectionError("failed".to_string()),
            RpcError::StreamError("closed".to_string()),
            RpcError::Timeout,
        ];

        for error in errors {
            let display = error.to_string();
            assert!(!display.is_empty());
        }
    }

    #[test]
    fn test_serialization_doctest() {
        let request = RpcRequest::new(1, "test".to_string(), vec![1, 2, 3]);
        let serialized = bincode::serialize(&request).unwrap();
        let deserialized: RpcRequest = bincode::deserialize(&serialized).unwrap();

        assert_eq!(request.id(), deserialized.id());
        assert_eq!(request.method(), deserialized.method());
        assert_eq!(request.params(), deserialized.params());
    }
}
