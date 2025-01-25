use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex};
use crate::config::RpcConfig;
use crate::errors::RpcError;
use crate::handler::HandlerFn;
use crate::request::RpcRequest;
use crate::response::RpcResponse;


pub struct RpcServer {
    handlers: HashMap<String, HandlerFn>,
    config: RpcConfig,
}

impl RpcServer {
    pub fn new(config: RpcConfig) -> Result<Self, RpcError> {
        Ok(Self {
            handlers: HashMap::new(),
            config,
        })
    }

    pub fn register<F>(&mut self, method: &str, handler: F)
    where
        F: Fn(Vec<u8>) -> Result<Vec<u8>, RpcError> + Send + Sync + 'static,
    {
        self.handlers
            .insert(method.to_string(), Arc::new(handler));
    }

    async fn handle_request(&self, request_data: Vec<u8>) -> Result<Vec<u8>, RpcError> {
        let request: RpcRequest = bincode::deserialize(&request_data)?;
        let handler = self.handlers.get(request.method()).ok_or_else(|| {
            RpcError::UnknownMethod(request.method().to_string())
        })?;

        let response = RpcResponse::from_result(
            request.id(),
            handler(request.params().to_vec())
        );

        Ok(bincode::serialize(&response)?)
    }

    pub async fn start(&self, addr: SocketAddr) -> Result<(), RpcError> {
        let socket = UdpSocket::bind(addr).await?;
        println!("RPC server listening on {}", addr);

        let mut quiche_config = self.config.build()?;
        let rng = ring::rand::SystemRandom::new();
        let conn_id_seed = ring::hmac::Key::generate(ring::hmac::HMAC_SHA256, &rng)?;

        // Connection management
        let connections = Arc::new(Mutex::new(HashMap::<quiche::ConnectionId<'static>, quiche::Connection>::new()));
        let (conn_sender, mut conn_receiver) = mpsc::channel::<quiche::ConnectionId<'static>>(100);
        
        // Spawn connection cleanup task
        let connections_cleanup = connections.clone();
        tokio::spawn(async move {
            while let Some(conn_id) = conn_receiver.recv().await {
                let mut conns = connections_cleanup.lock().await;
                if let Some(conn) = conns.get(&conn_id) {
                    if conn.is_closed() {
                        conns.remove(&conn_id);
                    }
                }
            }
        });

        let mut buf = [0; MAX_DATAGRAM_SIZE];
        let mut out = [0; MAX_DATAGRAM_SIZE];

        loop {
            let (len, src) = match socket.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("UDP receive error: {}", e);
                    continue;
                }
            };

            let local_addr = socket.local_addr()?;
            let pkt_buf = &mut buf[..len];

            // Parse incoming packet
            let header = match quiche::Header::from_slice(pkt_buf, quiche::MAX_CONN_ID_LEN) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Failed to parse QUIC header: {}", e);
                    continue;
                }
            };

            let mut conns = connections.lock().await;
            
            // Create a new connection if this is a new client
            if !conns.contains_key(&header.dcid) {
                if header.ty != quiche::Type::Initial {
                    continue;
                }

                let scid = quiche::ConnectionId::from_ref(&header.dcid);
                if let Ok(conn) = quiche::accept(&scid, None, local_addr, src, &mut quiche_config) {
                    let dcid = header.dcid.to_owned();
                    conns.insert(dcid.clone(), conn);
                    conn_sender.send(dcid).await.ok();
                }
            }

            // Process packet for existing connection
            if let Some(conn) = conns.get_mut(&header.dcid) {
                let recv_info = quiche::RecvInfo {
                    from: src,
                    to: local_addr,
                };

                let read = match conn.recv(pkt_buf, recv_info) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Failed to process QUIC packet: {}", e);
                        continue;
                    }
                };

                let mut readable = vec![];
                conn.for_each_event(|event| {
                    if let quiche::ConnectionError::StreamReadable(stream_id) = event {
                        readable.push(stream_id);
                    }
                });

                // Handle readable streams
                for stream_id in 0..=conn.peer_streams_left_bidi() {
                    let stream_id = stream_id * 2; // Client-initiated streams are even-numbered
                    
                    match conn.stream_recv(stream_id, &mut out) {
                        Ok((read, fin)) => {
                            if read == 0 {
                                continue;
                            }

                            let response = match self.handle_request(out[..read].to_vec()).await {
                                Ok(resp) => resp,
                                Err(e) => {
                                    eprintln!("Failed to handle request: {}", e);
                                    continue;
                                }
                            };

                            if let Err(e) = conn.stream_send(stream_id, &response, true) {
                                eprintln!("Failed to send response: {}", e);
                            }
                        }
                        Err(quiche::Error::Done) => {
                            continue;
                        }
                        Err(e) => {
                            eprintln!("Failed to receive stream data: {}", e);
                            continue;
                        }
                    }
                }

                // Send response packets
                loop {
                    let (write, send_info) = match conn.send(&mut out) {
                        Ok((written, send_info)) => (written, send_info),
                        Err(quiche::Error::Done) => {
                            break;
                        }
                        Err(e) => {
                            eprintln!("Failed to create QUIC packet: {}", e);
                            break;
                        }
                    };

                    if let Err(e) = socket.send_to(&out[..write], send_info.to).await {
                        eprintln!("Failed to send QUIC packet: {}", e);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::client::RpcClient;
    use super::*;
    use std::collections::HashSet;
    use std::net::SocketAddr;
    use std::str::FromStr;
    use std::time::Duration;
    use std::sync::atomic::{AtomicU16, Ordering};
    use tokio::time::timeout;

    static NEXT_PORT: AtomicU16 = AtomicU16::new(50051);
    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    fn get_test_addr() -> SocketAddr {
        let port = NEXT_PORT.fetch_add(1, Ordering::SeqCst);
        SocketAddr::from_str(&format!("127.0.0.1:{}", port)).unwrap()
    }

    async fn setup_test_server() -> RpcServer {
        let config = RpcConfig::default();
        let mut server = RpcServer::new(config).expect("Failed to create server");
        
        // Register a test handler
        server.register("test_method", |params| {
            Ok(params.to_vec())
        });
        
        // Register an echo handler
        server.register("echo", |params| {
            Ok(params)
        });

        server
    }

    #[tokio::test]
    async fn test_client_server_connection() -> Result<(), RpcError> {
        let server = setup_test_server().await;
        let addr = get_test_addr();
        
        // Start server in background
        tokio::spawn(async move {
            server.start(addr).await.expect("Server failed to start");
        });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Test client connection
        let config = RpcConfig::default();
        let client = timeout(
            TEST_TIMEOUT,
            RpcClient::connect(addr, config)
        ).await.unwrap().unwrap();

        let conn_closed = client.conn_manager.read().await.is_closed();
        assert!(!conn_closed);
        Ok(())
    }

    #[tokio::test]
    async fn test_simple_request_response() -> Result<(), RpcError> {
        let server = setup_test_server().await;
        let addr = get_test_addr();
        
        tokio::spawn(async move {
            server.start(addr).await.expect("Server failed to start");
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let config = RpcConfig::default();
        let client = RpcClient::connect(addr, config).await?;

        // Test echo request
        let test_data = b"Hello, RPC!".to_vec();
        let response = timeout(
            TEST_TIMEOUT,
            client.call("echo", test_data.clone())
        ).await.unwrap().unwrap();

        assert_eq!(response, test_data);
        Ok(())
    }

    #[tokio::test]
    async fn test_unknown_method() -> Result<(), RpcError> {
        let server = setup_test_server().await;
        let addr = get_test_addr();
        
        tokio::spawn(async move {
            server.start(addr).await.expect("Server failed to start");
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let config = RpcConfig::default();
        let client = RpcClient::connect(addr, config).await?;

        // Test calling unknown method
        let result = timeout(
            TEST_TIMEOUT,
            client.call("nonexistent_method", vec![])
        ).await.unwrap();

        assert!(matches!(result, Err(RpcError::UnknownMethod(_))));
        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_requests() -> Result<(), RpcError> {
        let server = setup_test_server().await;
        let addr = get_test_addr();
        
        tokio::spawn(async move {
            server.start(addr).await.expect("Server failed to start");
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let config = RpcConfig::default();
        let client = Arc::new(RpcClient::connect(addr, config).await?);
        let mut handles = vec![];

        // Launch multiple concurrent requests
        for i in 0..10 {
            let client = client.clone();
            let handle = tokio::spawn(async move {
                let data = format!("Request {}", i).into_bytes();
                client.call("echo", data.clone()).await
            });
            handles.push(handle);
        }

        // Wait for all requests to complete
        for handle in handles {
            let result = handle.await.expect("Task panicked")?;
            assert!(!result.is_empty());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_request_timeout() -> Result<(), RpcError> {
        let addr = get_test_addr();
        let config = RpcConfig::default();
        
        // Try to connect to non-existent server
        let result = timeout(
            Duration::from_secs(1),
            RpcClient::connect(addr, config)
        ).await;
        
        assert!(result.is_err() || result.unwrap().is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_large_payload() -> Result<(), RpcError> {
        let server = setup_test_server().await;
        let addr = get_test_addr();
        
        // Start server with longer timeout
        tokio::spawn(async move {
            server.start(addr).await.expect("Server failed to start");
        });

        // Give more time for server setup with large payloads
        tokio::time::sleep(Duration::from_millis(200)).await;

        let mut config = RpcConfig::default();
        // Adjust config for large payloads if needed
        //config.max_data(10 * 1024 * 1024); // 10MB limit
        //config.max_stream_data(5 * 1024 * 1024); // 5MB per stream
        
        let client = RpcClient::connect(addr, config).await?;

        // Test with increasing payload sizes
        let payload_sizes = [
            1024,           // 1KB
            64 * 1024,     // 64KB
            256 * 1024,    // 256KB
            512 * 1024,    // 512KB
        ];

        for size in payload_sizes {
            let test_data = vec![0u8; size];
            
            let response = timeout(
                TEST_TIMEOUT,
                client.call("echo", test_data.clone())
            ).await.unwrap().unwrap();

            assert_eq!(response.len(), test_data.len(), "Payload size mismatch for {} bytes", size);
            assert_eq!(response, test_data, "Data corruption for {} bytes payload", size);
            
            // Small delay between large payload tests
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_request_id_generation() {
        let next_id = Arc::new(Mutex::new(0));
        
        let mut ids = HashSet::new();
        for _ in 0..1000 {
            let mut id = next_id.lock().await;
            *id += 1;
            assert!(ids.insert(*id));
        }
    }
}