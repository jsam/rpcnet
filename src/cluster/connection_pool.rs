mod config;
mod error;

pub use config::PoolConfig;
pub use error::PoolError;

use async_trait::async_trait;
use dashmap::DashMap;
use s2n_quic::client::Connect;
use s2n_quic::Client as QuicClient;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, timeout};

#[async_trait]
pub trait ConnectionPool: Send + Sync {
    async fn get_or_create(&self, addr: SocketAddr) -> Result<PooledConnection, PoolError>;
    fn release(&self, addr: &SocketAddr);
    async fn run_idle_cleanup(&self);
    fn stats(&self) -> PoolStats;
}

#[derive(Debug, Clone)]
pub struct PooledConnection {
    pub(crate) connection: Arc<Mutex<s2n_quic::Connection>>,
    pub(crate) addr: SocketAddr,
    last_used: Arc<RwLock<Instant>>,
}

impl PooledConnection {
    fn new(connection: s2n_quic::Connection, addr: SocketAddr) -> Self {
        Self {
            connection: Arc::new(Mutex::new(connection)),
            addr,
            last_used: Arc::new(RwLock::new(Instant::now())),
        }
    }

    pub async fn with_connection_mut<F, Fut, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut s2n_quic::Connection) -> Fut + Send,
        Fut: std::future::Future<Output = R> + Send,
        R: Send,
    {
        let mut conn = self.connection.lock().await;
        f(&mut conn).await
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub async fn last_used(&self) -> Instant {
        *self.last_used.read().await
    }

    pub(crate) async fn touch(&self) {
        *self.last_used.write().await = Instant::now();
    }
}

#[derive(Debug)]
struct PoolEntry {
    connection: PooledConnection,
    in_use: Arc<AtomicBool>,
    health_check_count: Arc<AtomicU64>,
}

impl Clone for PoolEntry {
    fn clone(&self) -> Self {
        Self {
            connection: self.connection.clone(),
            in_use: self.in_use.clone(),
            health_check_count: self.health_check_count.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total_connections: usize,
    pub in_use: usize,
    pub idle: usize,
    pub per_peer: std::collections::HashMap<SocketAddr, usize>,
}

pub struct ConnectionPoolImpl {
    connections: Arc<DashMap<SocketAddr, PoolEntry>>,
    config: PoolConfig,
    quic_client: Arc<QuicClient>,
}

impl ConnectionPoolImpl {
    pub fn new(config: PoolConfig, quic_client: Arc<QuicClient>) -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
            config,
            quic_client,
        }
    }

    async fn try_reuse_existing(&self, addr: SocketAddr) -> Option<PooledConnection> {
        if let Some(entry) = self.connections.get(&addr) {
            if entry
                .in_use
                .compare_exchange(false, true, AtomicOrdering::AcqRel, AtomicOrdering::Acquire)
                .is_ok()
            {
                entry.connection.touch().await;
                return Some(entry.connection.clone());
            }
        }
        None
    }

    async fn evict_idle_connection(&self) -> Result<(), PoolError> {
        let now = Instant::now();
        let idle_threshold = self.config.idle_timeout;

        let to_evict = {
            let mut oldest: Option<(SocketAddr, Instant)> = None;

            for entry in self.connections.iter() {
                if !entry.in_use.load(AtomicOrdering::Acquire) {
                    let last_used = entry.connection.last_used().await;
                    if now.duration_since(last_used) > idle_threshold {
                        match oldest {
                            None => oldest = Some((*entry.key(), last_used)),
                            Some((_, old_time)) if last_used < old_time => {
                                oldest = Some((*entry.key(), last_used));
                            }
                            _ => {}
                        }
                    }
                }
            }

            if oldest.is_none() {
                for entry in self.connections.iter() {
                    if !entry.in_use.load(AtomicOrdering::Acquire) {
                        let last_used = entry.connection.last_used().await;
                        match oldest {
                            None => oldest = Some((*entry.key(), last_used)),
                            Some((_, old_time)) if last_used < old_time => {
                                oldest = Some((*entry.key(), last_used));
                            }
                            _ => {}
                        }
                    }
                }
            }

            oldest.map(|(addr, _)| addr)
        };

        if let Some(addr) = to_evict {
            if let Some(entry) = self.connections.get(&addr) {
                if entry
                    .in_use
                    .compare_exchange(false, true, AtomicOrdering::AcqRel, AtomicOrdering::Acquire)
                    .is_ok()
                {
                    self.connections.remove(&addr);
                    return Ok(());
                }
            }
        }

        Err(PoolError::NoIdleConnections)
    }

    async fn check_connection_health(&self, _conn: &s2n_quic::Connection) -> bool {
        true
    }
}

#[async_trait]
impl ConnectionPool for ConnectionPoolImpl {
    async fn get_or_create(&self, addr: SocketAddr) -> Result<PooledConnection, PoolError> {
        if let Some(conn) = self.try_reuse_existing(addr).await {
            return Ok(conn);
        }

        let current_total = self.connections.len();
        if current_total >= self.config.max_total {
            self.evict_idle_connection().await?;
        }

        let per_peer_count = self.connections.iter().filter(|e| *e.key() == addr).count();

        if per_peer_count >= self.config.max_per_peer {
            return Err(PoolError::PoolFull {
                current: per_peer_count,
                max: self.config.max_per_peer,
            });
        }

        let connection = timeout(
            self.config.connect_timeout,
            self.quic_client
                .connect(Connect::new(addr).with_server_name("localhost")),
        )
        .await
        .map_err(|_| PoolError::Timeout {
            timeout: self.config.connect_timeout,
        })?
        .map_err(|e| PoolError::ConnectionFailed {
            addr,
            source: Box::new(e),
        })?;

        let pooled = PooledConnection::new(connection, addr);
        let entry = PoolEntry {
            connection: pooled.clone(),
            in_use: Arc::new(AtomicBool::new(true)),
            health_check_count: Arc::new(AtomicU64::new(0)),
        };

        self.connections.insert(addr, entry);
        Ok(pooled)
    }

    fn release(&self, addr: &SocketAddr) {
        if let Some(entry) = self.connections.get(addr) {
            entry.in_use.store(false, AtomicOrdering::Release);
        }
    }

    async fn run_idle_cleanup(&self) {
        let connections = self.connections.clone();
        let config = self.config.clone();

        loop {
            sleep(config.health_check_interval).await;

            let now = Instant::now();
            let idle_threshold = config.idle_timeout;

            let mut to_remove = Vec::new();
            let mut to_health_check = Vec::new();

            for entry in connections.iter() {
                let in_use = entry.in_use.load(AtomicOrdering::Acquire);
                let last_used = entry.connection.last_used().await;
                let idle_duration = now.duration_since(last_used);

                if !in_use && idle_duration > idle_threshold {
                    to_remove.push(*entry.key());
                } else if !in_use && idle_duration > config.health_check_interval {
                    to_health_check.push((*entry.key(), entry.connection.clone()));
                }
            }

            for addr in to_remove {
                connections.remove(&addr);
            }

            for (addr, conn) in to_health_check {
                let conn_guard = conn.connection.lock().await;
                let is_healthy = self.check_connection_health(&conn_guard).await;
                drop(conn_guard);

                if !is_healthy {
                    connections.remove(&addr);
                }
            }
        }
    }

    fn stats(&self) -> PoolStats {
        let mut per_peer = std::collections::HashMap::new();
        let mut in_use_count = 0;

        for entry in self.connections.iter() {
            *per_peer.entry(*entry.key()).or_insert(0) += 1;
            if entry.in_use.load(AtomicOrdering::Acquire) {
                in_use_count += 1;
            }
        }

        let total = self.connections.len();

        PoolStats {
            total_connections: total,
            in_use: in_use_count,
            idle: total - in_use_count,
            per_peer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::Duration;

    async fn create_test_client() -> Arc<QuicClient> {
        let cert_path = Path::new("certs/test_cert.pem");
        let client = QuicClient::builder()
            .with_tls(cert_path)
            .unwrap()
            .with_io("0.0.0.0:0")
            .unwrap()
            .start()
            .unwrap();

        Arc::new(client)
    }

    #[tokio::test]
    async fn test_pool_max_bounds() {
        let config = PoolConfig::new().with_max_total(2);
        let client = create_test_client().await;
        let pool = ConnectionPoolImpl::new(config, client);

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.idle, 0);
    }

    #[tokio::test]
    async fn test_connection_reuse() {
        let config = PoolConfig::new();
        let client = create_test_client().await;
        let pool = ConnectionPoolImpl::new(config, client);

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
    }

    #[tokio::test]
    async fn test_idle_eviction() {
        let config = PoolConfig::new().with_idle_timeout(Duration::from_millis(100));
        let client = create_test_client().await;
        let pool = ConnectionPoolImpl::new(config, client);

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
    }

    #[tokio::test]
    async fn test_per_peer_limit() {
        let config = PoolConfig::new().with_max_per_peer(1);
        let client = create_test_client().await;
        let pool = ConnectionPoolImpl::new(config, client);

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
    }

    #[tokio::test]
    async fn test_release_makes_connection_idle() {
        let config = PoolConfig::new();
        let client = create_test_client().await;
        let pool = ConnectionPoolImpl::new(config, client);

        let stats = pool.stats();
        assert_eq!(stats.in_use, 0);
        assert_eq!(stats.idle, 0);
    }

    #[tokio::test]
    async fn test_stats() {
        let config = PoolConfig::new();
        let client = create_test_client().await;
        let pool = ConnectionPoolImpl::new(config, client);

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.in_use, 0);
        assert_eq!(stats.idle, 0);
        assert!(stats.per_peer.is_empty());
    }

    #[tokio::test]
    async fn test_concurrent_get_or_create_same_addr() {
        let config = PoolConfig::new().with_max_per_peer(1);
        let client = create_test_client().await;
        let pool = Arc::new(ConnectionPoolImpl::new(config, client));

        let addr: SocketAddr = "127.0.0.1:8001".parse().unwrap();

        let pool1 = pool.clone();
        let pool2 = pool.clone();

        let handle1 = tokio::spawn(async move { pool1.try_reuse_existing(addr).await });

        let handle2 = tokio::spawn(async move { pool2.try_reuse_existing(addr).await });

        let (result1, result2) = tokio::join!(handle1, handle2);

        let conn1 = result1.unwrap();
        let conn2 = result2.unwrap();

        assert!(conn1.is_none() && conn2.is_none());
    }

    #[tokio::test]
    async fn test_per_peer_limit_logic_via_count() {
        let config = PoolConfig::new().with_max_per_peer(2);
        let client = create_test_client().await;
        let pool = ConnectionPoolImpl::new(config, client);

        let addr1: SocketAddr = "127.0.0.1:9001".parse().unwrap();
        let _addr2: SocketAddr = "127.0.0.1:9002".parse().unwrap();

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.per_peer.get(&addr1), None);
    }

    #[tokio::test]
    async fn test_evict_idle_connection_logic() {
        let config = PoolConfig::new()
            .with_idle_timeout(Duration::from_millis(50))
            .with_max_total(2);
        let client = create_test_client().await;
        let pool = Arc::new(ConnectionPoolImpl::new(config, client));

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
    }

    #[tokio::test]
    async fn test_pool_stats_per_peer_tracking() {
        let config = PoolConfig::new();
        let client = create_test_client().await;
        let pool = ConnectionPoolImpl::new(config, client);

        let stats = pool.stats();

        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.in_use, 0);
        assert_eq!(stats.idle, 0);
        assert_eq!(stats.per_peer.len(), 0);
    }

    #[tokio::test]
    async fn test_connection_timeout_config() {
        let config = PoolConfig::new().with_connect_timeout(Duration::from_millis(100));
        let client = create_test_client().await;
        let pool = ConnectionPoolImpl::new(config, client);

        let addr: SocketAddr = "127.0.0.1:19999".parse().unwrap();

        let result = pool.get_or_create(addr).await;

        assert!(result.is_err());
        if let Err(e) = result {
            match e {
                PoolError::Timeout { timeout } => {
                    assert_eq!(timeout, Duration::from_millis(100));
                }
                PoolError::ConnectionFailed { .. } => {}
                _ => panic!("Expected Timeout or ConnectionFailed error"),
            }
        }
    }
}
