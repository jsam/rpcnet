use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    future::Future,
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use thiserror::Error;
use tokio::sync::RwLock;
use s2n_quic::{client::Connect, Client};

/// By default 30s in normal code, but shorten to 2s in tests for quick timeouts.
#[cfg(not(test))]
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(test)]
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

// ==========================
//         Errors
// ==========================
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
}

// ==========================
//       RPC Request
// ==========================
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

// ==========================
//      RPC Response
// ==========================
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

// ==========================
//       RPC Config
// ==========================
#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub cert_path: PathBuf,
    pub key_path: Option<PathBuf>,
    pub server_name: String,
    pub bind_address: String,
    pub keep_alive_interval: Option<Duration>,
}

impl RpcConfig {
    pub fn new<P: Into<PathBuf>>(cert_path: P, bind_address: impl Into<String>) -> Self {
        Self {
            cert_path: cert_path.into(),
            key_path: None,
            server_name: "localhost".to_string(),
            bind_address: bind_address.into(),
            keep_alive_interval: Some(Duration::from_secs(30)),
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
}

// ==========================
//    Async Handler Fn
// ==========================
type AsyncHandlerFn = Box<
    dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, RpcError>> + Send>>
        + Send
        + Sync,
>;

// ==========================
//       RPC Server
// ==========================
#[derive(Clone)]
pub struct RpcServer {
    pub handlers: Arc<RwLock<HashMap<String, AsyncHandlerFn>>>,
    pub socket_addr: Option<SocketAddr>,
    pub config: RpcConfig
}

impl RpcServer {
    pub fn new(config: RpcConfig) -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            socket_addr: None,
            config,
        }
    }

    

    /// Register a new async RPC method handler
    pub async fn register<F, Fut>(&self, method: &str, handler: F)
    where
        F: Fn(Vec<u8>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<u8>, RpcError>> + Send + 'static,
    {
        let mut handlers = self.handlers.write().await;
        handlers.insert(
            method.to_string(),
            Box::new(move |params: Vec<u8>| {
                Box::pin(handler(params)) as Pin<Box<dyn Future<Output=_> + Send>>
            }),
        );
    }

    /// Start server (blocks in accept loop).
    pub async fn start(&mut self, mut server: s2n_quic::Server) -> Result<(), RpcError> {
        //let mut server = self.bind()?;
        
        while let Some(mut connection) = server.accept().await {
            let handlers = self.handlers.clone();
            tokio::spawn(async move {
                // For each accepted connection, keep accepting streams:
                while let Ok(Some(mut stream)) = connection.accept_bidirectional_stream().await {
                    let handlers = handlers.clone();
                    tokio::spawn(async move {
                        let mut request_data = Vec::new();
                        while let Ok(Some(data)) = stream.receive().await {
                            request_data.extend_from_slice(&data);
                            // Attempt to parse request:
                            if let Ok(request) = bincode::deserialize::<RpcRequest>(&request_data) {
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
                                    let _ = stream.send(response_data.into()).await;
                                }
                                break; // Handle one request per stream
                            }
                        }
                    });
                }
            });
        }
        
        Ok(())
    }

    pub fn bind(&mut self) -> Result<s2n_quic::Server, RpcError> {
        let key_path = self
            .config
            .key_path
            .as_ref()
            .ok_or_else(|| RpcError::ConfigError("Server key path not configured".to_string()))?;
        
        let mut server = s2n_quic::Server::builder()
            .with_tls((self.config.cert_path.as_path(), key_path.as_path()))
            .map_err(|e| RpcError::TlsError(format!("{:?}", e)))?
            .with_io(self.config.bind_address.as_str())
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?
            .start()
            .map_err(|e| RpcError::ConfigError(format!("{:?}", e)))?;
        
        let local_addr = server.local_addr().map_err(|err| {
            RpcError::ConfigError("Could not retrieve local_addr() from server".to_string())
        })?;
                
        self.socket_addr = Some(local_addr.clone());
        println!("RPC server listening on {local_addr}");
        Ok(server)
    }
}
        
// ==========================
//       RPC Client
// ==========================
pub struct RpcClient {
    connection: Arc<RwLock<s2n_quic::Connection>>,
    pub next_id: Arc<AtomicU64>,
}

impl RpcClient {
    /// Connect to an RPC server
    pub async fn connect(connect_addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        let client = Client::builder()
            .with_tls(config.cert_path.as_path())
            .map_err(|e| RpcError::TlsError(format!("{:?}", e)))?
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
            connection: Arc::new(RwLock::new(connection)),
            next_id: Arc::new(AtomicU64::new(1)),
        })
    }

    /// Make an RPC call (one request per stream).
    pub async fn call(&self, method: &str, params: Vec<u8>) -> Result<Vec<u8>, RpcError> {
        // Generate a new request ID
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = RpcRequest::new(id, method.to_string(), params);
        let req_data = bincode::serialize(&req)?;

        // Open a new bidirectional stream
        let mut conn = self.connection.write().await;
        let mut stream = conn
            .open_bidirectional_stream()
            .await
            .map_err(|e| RpcError::StreamError(e.to_string()))?;

        // Send the request
        stream
            .send(req_data.into())
            .await
            .map_err(|e| RpcError::StreamError(e.to_string()))?;

        // Read back the response (with an overall timeout)
        let read_future = async {
            let mut response_data = Vec::new();
            while let Ok(Some(chunk)) = stream.receive().await {
                response_data.extend_from_slice(&chunk);
                // Try to parse a complete response
                if let Ok(response) = bincode::deserialize::<RpcResponse>(&response_data) {
                    if response.id() == id {
                        return match (response.result(), response.error()) {
                            (Some(data), None) => Ok(data.clone()),
                            (None, Some(err_msg)) => Err(RpcError::StreamError(err_msg.clone())),
                            _ => Err(RpcError::StreamError("Invalid response".to_string())),
                        };
                    }
                }
            }
            // If we exit the loop without returning, the stream closed early or never gave a valid response
            Err(RpcError::ConnectionError("Stream closed unexpectedly".to_string()))
        };

        // Enforce the DEFAULT_TIMEOUT
        match tokio::time::timeout(DEFAULT_TIMEOUT, read_future).await {
            Ok(res) => res,
            Err(_) => Err(RpcError::Timeout),
        }
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

    /// Create a real QUIC server on ephemeral port; returns (addr, join_handle)
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
                    while let Ok(Some(mut stream)) = connection.accept_bidirectional_stream().await {
                        let handlers = handlers.clone();
                        tokio::spawn(async move {
                            let mut request_data = Vec::new();
                            while let Ok(Some(data)) = stream.receive().await {
                                request_data.extend_from_slice(&data);
                                if let Ok(request) = bincode::deserialize::<RpcRequest>(&request_data)
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
            tasks.push(tokio::spawn(async move {
                c.call("test_method", vec![]).await
            }));
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
}
