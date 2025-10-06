use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub max_per_peer: usize,
    pub max_total: usize,
    pub idle_timeout: Duration,
    pub connect_timeout: Duration,
    pub health_check_interval: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_per_peer: 1,
            max_total: 50,
            idle_timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(30),
        }
    }
}

impl PoolConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_per_peer(mut self, max: usize) -> Self {
        self.max_per_peer = max;
        self
    }

    pub fn with_max_total(mut self, max: usize) -> Self {
        self.max_total = max;
        self
    }

    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn with_health_check_interval(mut self, interval: Duration) -> Self {
        self.health_check_interval = interval;
        self
    }
}
