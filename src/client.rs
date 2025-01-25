use std::{collections::HashMap, net::SocketAddr, sync::{Arc, Mutex}};

use tokio::sync::{oneshot, RwLock};

use crate::{config::RpcConfig, connection::ConnectionManager, errors::RpcError, request::RpcRequest, response::RpcResponse, MAX_DATAGRAM_SIZE};


// RPC Client implementation
pub struct RpcClient {
    pub conn_manager: Arc<RwLock<ConnectionManager>>,
    next_id: Arc<Mutex<u64>>,
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Vec<u8>, RpcError>>>>>,
}

impl RpcClient {
    pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        let conn_manager = ConnectionManager::connect(addr, &config).await?;
        
        Ok(Self {
            conn_manager: Arc::new(RwLock::new(conn_manager)),
            next_id: Arc::new(Mutex::new(0)),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn call(&self, method: &str, params: Vec<u8>) -> Result<Vec<u8>, RpcError> {
        let id = {
            let mut id = self.next_id.lock().unwrap();
            *id += 1;
            *id
        };

        let request = RpcRequest::new(
            id,
            method.to_string(),
            params,
        );

        let request_data = bincode::serialize(&request)?;

        let (tx, rx) = oneshot::channel();
        {
            self.pending_requests.lock().unwrap().insert(id, tx);
        }

        self.conn_manager.write().await.stream_send(id as u64, &request_data, true)?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err(RpcError::Timeout),
        }
    }

    pub fn handle_response(&self, response_data: &[u8] ) -> Result<(), RpcError> {
        let response: RpcResponse = bincode::deserialize(&response_data)?;
        
        if let Some(tx) = self.pending_requests.lock().unwrap().remove(&response.id()) {
            if let Some(error) = response.error() {
                let _ = tx.send(Err(RpcError::ConnectionError(error.clone())));
            } else if let Some(result) = response.result() {
                let _ = tx.send(Ok(result.clone()));
            }
        }
        
        Ok(())
    }

    async fn receive_loop(&self) {
        let mut buf = [0; MAX_DATAGRAM_SIZE];
        loop {
            let mut manager = self.conn_manager.read().await;
            match manager.dgram_recv(&mut buf) {
                Ok(size) => {
                    let pkt = &buf[..size];
                    // Process the QUIC packet
                    // if let Ok(response_data) = self.conn_manager.read().await.process_packet(pkt).await {
                    //     let _ = self.handle_response(response_data);
                    // }
                    let _ = self.handle_response(pkt);
                }
                Err(e) => {
                    eprintln!("Client receive error: {}", e);
                }
            }
        }
    }
}