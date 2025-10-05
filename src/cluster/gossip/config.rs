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
