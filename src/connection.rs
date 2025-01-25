use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use rand::Rng;

use crate::{config::RpcConfig, errors::RpcError};


// Connection manager for handling QUIC connections
pub struct ConnectionManager {
    conn: quiche::Connection,
    pub config: RpcConfig,
}

impl ConnectionManager {
    pub fn new(conn: quiche::Connection, config: RpcConfig) -> Self {
        Self { conn, config }
    }

    fn determine_local_addr(peer_addr: SocketAddr) -> SocketAddr {
        // Choose appropriate local IP based on peer IP version
        let local_ip = match peer_addr.ip() {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),  // 0.0.0.0
            IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),  // ::
        };
        
        // Use port 0 to let OS assign an available port
        SocketAddr::new(local_ip, 0)
    }

    pub fn stream_send(
        &mut self, stream_id: u64, buf: &[u8], fin: bool,
    ) -> Result<usize, RpcError> {
        self.conn.stream_send(stream_id, buf, fin)
            .map(|v| v as usize)
            .map_err(|e| RpcError::ConnectionError(e.to_string()))
    }

    pub fn dgram_recv(&mut self, buf: &mut [u8]) -> Result<usize, RpcError> {
        self.conn.dgram_recv(buf)
            .map(|v| v as usize)
            .map_err(|e| RpcError::ConnectionError(e.to_string()))
    }

    pub async fn connect(peer_addr: SocketAddr, config: &RpcConfig) -> Result<Self, RpcError> {
        let mut quiche_config = config.build()?;
        
        // Generate a random source connection ID. The `scid` parameter is used as the connection's source connection ID
        let mut scid = [0; 16];
        rand::thread_rng().fill(&mut scid[..]);
        let scid = quiche::ConnectionId::from_ref(&scid);

         // Determine appropriate local address based on peer
         let local_addr = Self::determine_local_addr(peer_addr);

        let conn = quiche::connect(
            None,
            &scid,
            local_addr,   
            peer_addr,
            &mut quiche_config,
        )?;

        Ok(Self::new(conn, config.clone()))
    }

    pub fn is_closed(&self) -> bool {
        self.conn.is_closed()
    }

    pub fn is_established(&self) -> bool {
        self.conn.is_established()
    }

}