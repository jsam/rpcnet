use thiserror::Error;

#[derive(Debug, Error)]
pub enum PoolError {
    #[error("Pool is at maximum capacity ({current}/{max})")]
    PoolFull { current: usize, max: usize },

    #[error("Connection to {addr} failed: {source}")]
    ConnectionFailed {
        addr: std::net::SocketAddr,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Connection timed out after {timeout:?}")]
    Timeout { timeout: std::time::Duration },

    #[error("No idle connections available for eviction")]
    NoIdleConnections,

    #[error("Connection to {addr} is unhealthy")]
    UnhealthyConnection { addr: std::net::SocketAddr },
}
