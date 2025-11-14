use std::time::Duration;

#[derive(Debug, Clone)]
pub struct GossipConfig {
    pub protocol_period: Duration,
    pub indirect_ping_count: usize,
    pub ack_timeout: Duration,
    pub indirect_timeout: Duration,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            protocol_period: Duration::from_secs(1),
            indirect_ping_count: 3,
            ack_timeout: Duration::from_millis(500),
            indirect_timeout: Duration::from_millis(1000),
        }
    }
}

impl GossipConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_protocol_period(mut self, period: Duration) -> Self {
        self.protocol_period = period;
        self
    }

    pub fn with_indirect_ping_count(mut self, count: usize) -> Self {
        self.indirect_ping_count = count;
        self
    }

    pub fn with_ack_timeout(mut self, timeout: Duration) -> Self {
        self.ack_timeout = timeout;
        self
    }

    pub fn with_indirect_timeout(mut self, timeout: Duration) -> Self {
        self.indirect_timeout = timeout;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = GossipConfig::default();
        assert_eq!(config.protocol_period, Duration::from_secs(1));
        assert_eq!(config.indirect_ping_count, 3);
        assert_eq!(config.ack_timeout, Duration::from_millis(500));
        assert_eq!(config.indirect_timeout, Duration::from_millis(1000));
    }

    #[test]
    fn test_config_new() {
        let config = GossipConfig::new();
        assert_eq!(config.protocol_period, Duration::from_secs(1));
        assert_eq!(config.indirect_ping_count, 3);
    }

    #[test]
    fn test_config_builder_pattern() {
        let config = GossipConfig::new()
            .with_protocol_period(Duration::from_millis(500))
            .with_indirect_ping_count(5)
            .with_ack_timeout(Duration::from_millis(200))
            .with_indirect_timeout(Duration::from_millis(800));

        assert_eq!(config.protocol_period, Duration::from_millis(500));
        assert_eq!(config.indirect_ping_count, 5);
        assert_eq!(config.ack_timeout, Duration::from_millis(200));
        assert_eq!(config.indirect_timeout, Duration::from_millis(800));
    }

    #[test]
    fn test_config_partial_builder() {
        // Test that we can set some values while keeping others at default
        let config = GossipConfig::default().with_protocol_period(Duration::from_millis(100));

        assert_eq!(config.protocol_period, Duration::from_millis(100));
        assert_eq!(config.indirect_ping_count, 3); // Still default
        assert_eq!(config.ack_timeout, Duration::from_millis(500)); // Still default
    }

    #[test]
    fn test_config_cloneable() {
        let config1 = GossipConfig::new().with_protocol_period(Duration::from_millis(200));
        let config2 = config1.clone();

        assert_eq!(config1.protocol_period, config2.protocol_period);
        assert_eq!(config1.indirect_ping_count, config2.indirect_ping_count);
    }
}
