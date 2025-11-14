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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_per_peer, 1);
        assert_eq!(config.max_total, 50);
        assert_eq!(config.idle_timeout, Duration::from_secs(60));
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.health_check_interval, Duration::from_secs(30));
    }

    #[test]
    fn test_pool_config_new() {
        let config = PoolConfig::new();
        assert_eq!(config.max_per_peer, 1);
        assert_eq!(config.max_total, 50);
        assert_eq!(config.idle_timeout, Duration::from_secs(60));
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.health_check_interval, Duration::from_secs(30));
    }

    #[test]
    fn test_with_max_per_peer() {
        let config = PoolConfig::new().with_max_per_peer(5);
        assert_eq!(config.max_per_peer, 5);
        assert_eq!(config.max_total, 50); // Other fields should remain default
    }

    #[test]
    fn test_with_max_total() {
        let config = PoolConfig::new().with_max_total(100);
        assert_eq!(config.max_total, 100);
        assert_eq!(config.max_per_peer, 1); // Other fields should remain default
    }

    #[test]
    fn test_with_idle_timeout() {
        let config = PoolConfig::new().with_idle_timeout(Duration::from_secs(120));
        assert_eq!(config.idle_timeout, Duration::from_secs(120));
        assert_eq!(config.max_per_peer, 1); // Other fields should remain default
    }

    #[test]
    fn test_with_connect_timeout() {
        let config = PoolConfig::new().with_connect_timeout(Duration::from_secs(10));
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.max_per_peer, 1); // Other fields should remain default
    }

    #[test]
    fn test_with_health_check_interval() {
        let config = PoolConfig::new().with_health_check_interval(Duration::from_secs(60));
        assert_eq!(config.health_check_interval, Duration::from_secs(60));
        assert_eq!(config.max_per_peer, 1); // Other fields should remain default
    }

    #[test]
    fn test_builder_pattern_chaining() {
        let config = PoolConfig::new()
            .with_max_per_peer(10)
            .with_max_total(200)
            .with_idle_timeout(Duration::from_secs(180))
            .with_connect_timeout(Duration::from_secs(15))
            .with_health_check_interval(Duration::from_secs(45));

        assert_eq!(config.max_per_peer, 10);
        assert_eq!(config.max_total, 200);
        assert_eq!(config.idle_timeout, Duration::from_secs(180));
        assert_eq!(config.connect_timeout, Duration::from_secs(15));
        assert_eq!(config.health_check_interval, Duration::from_secs(45));
    }

    #[test]
    fn test_pool_config_clone() {
        let config1 = PoolConfig::new().with_max_per_peer(3);
        let config2 = config1.clone();

        assert_eq!(config1.max_per_peer, config2.max_per_peer);
        assert_eq!(config1.max_total, config2.max_total);
        assert_eq!(config1.idle_timeout, config2.idle_timeout);
        assert_eq!(config1.connect_timeout, config2.connect_timeout);
        assert_eq!(config1.health_check_interval, config2.health_check_interval);
    }

    #[test]
    fn test_pool_config_debug() {
        let config = PoolConfig::new();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("PoolConfig"));
        assert!(debug_str.contains("max_per_peer"));
        assert!(debug_str.contains("max_total"));
    }
}
