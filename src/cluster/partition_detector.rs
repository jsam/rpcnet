use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct PartitionConfig {
    pub threshold: f64,
    pub grace_period: Duration,
    pub read_only_on_partition: bool,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            grace_period: Duration::from_secs(30),
            read_only_on_partition: false,
        }
    }
}

impl PartitionConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    pub fn with_grace_period(mut self, period: Duration) -> Self {
        self.grace_period = period;
        self
    }

    pub fn with_read_only_on_partition(mut self, read_only: bool) -> Self {
        self.read_only_on_partition = read_only;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartitionStatus {
    Unknown,
    Healthy {
        current_size: usize,
        expected_size: usize,
    },
    Suspect {
        current_size: usize,
        expected_size: usize,
        grace_remaining: Duration,
    },
    Partitioned {
        current_size: usize,
        expected_size: usize,
        minority: bool,
    },
}

pub struct PartitionDetector {
    expected_size: Arc<AtomicUsize>,
    config: PartitionConfig,
    detection_start: Arc<RwLock<Option<Instant>>>,
}

impl Clone for PartitionDetector {
    fn clone(&self) -> Self {
        Self {
            expected_size: self.expected_size.clone(),
            config: self.config.clone(),
            detection_start: self.detection_start.clone(),
        }
    }
}

impl PartitionDetector {
    pub fn new(config: PartitionConfig) -> Self {
        Self {
            expected_size: Arc::new(AtomicUsize::new(0)),
            config,
            detection_start: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn check_partition(&self, current_size: usize) -> PartitionStatus {
        let expected = self.expected_size.load(Ordering::SeqCst);

        if expected == 0 {
            return PartitionStatus::Unknown;
        }

        let threshold_size = (expected as f64 * self.config.threshold).ceil() as usize;

        if current_size < threshold_size {
            let now = Instant::now();
            let start_time = {
                let mut detection_start = self.detection_start.write().unwrap();
                *detection_start.get_or_insert(now)
            };

            let elapsed = now.duration_since(start_time);

            if elapsed >= self.config.grace_period {
                let minority = current_size < (expected / 2);
                PartitionStatus::Partitioned {
                    current_size,
                    expected_size: expected,
                    minority,
                }
            } else {
                let grace_remaining = self.config.grace_period - elapsed;
                PartitionStatus::Suspect {
                    current_size,
                    expected_size: expected,
                    grace_remaining,
                }
            }
        } else {
            let mut detection_start = self.detection_start.write().unwrap();
            *detection_start = None;

            PartitionStatus::Healthy {
                current_size,
                expected_size: expected,
            }
        }
    }

    pub fn update_expected_size(&self, size: usize) {
        self.expected_size.store(size, Ordering::SeqCst);
    }

    pub fn config(&self) -> &PartitionConfig {
        &self.config
    }

    pub fn expected_size(&self) -> usize {
        self.expected_size.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_healthy_cluster() {
        let detector = PartitionDetector::new(PartitionConfig::default());
        detector.update_expected_size(10);

        let status = detector.check_partition(10).await;

        assert_eq!(
            status,
            PartitionStatus::Healthy {
                current_size: 10,
                expected_size: 10
            }
        );
    }

    #[tokio::test]
    async fn test_suspect_below_threshold() {
        let config = PartitionConfig::default().with_grace_period(Duration::from_secs(1));
        let detector = PartitionDetector::new(config);
        detector.update_expected_size(10);

        let status = detector.check_partition(4).await;

        match status {
            PartitionStatus::Suspect {
                current_size,
                expected_size,
                ..
            } => {
                assert_eq!(current_size, 4);
                assert_eq!(expected_size, 10);
            }
            _ => panic!("Expected Suspect status"),
        }
    }

    #[tokio::test]
    async fn test_partition_after_grace_period() {
        let config = PartitionConfig::default().with_grace_period(Duration::from_millis(100));
        let detector = PartitionDetector::new(config);
        detector.update_expected_size(10);

        let status = detector.check_partition(4).await;
        assert!(matches!(status, PartitionStatus::Suspect { .. }));

        sleep(Duration::from_millis(150)).await;

        let status = detector.check_partition(4).await;
        match status {
            PartitionStatus::Partitioned {
                current_size,
                expected_size,
                minority,
            } => {
                assert_eq!(current_size, 4);
                assert_eq!(expected_size, 10);
                assert!(minority);
            }
            _ => panic!("Expected Partitioned status"),
        }
    }

    #[tokio::test]
    async fn test_minority_detection() {
        let config = PartitionConfig::default().with_grace_period(Duration::from_millis(100));
        let detector = PartitionDetector::new(config);
        detector.update_expected_size(10);

        detector.check_partition(3).await;
        sleep(Duration::from_millis(150)).await;

        let status = detector.check_partition(3).await;
        match status {
            PartitionStatus::Partitioned {
                minority,
                current_size,
                ..
            } => {
                assert_eq!(current_size, 3);
                assert!(minority);
            }
            _ => panic!("Expected Partitioned with minority=true"),
        }
    }

    #[tokio::test]
    async fn test_recovery_from_suspect() {
        let config = PartitionConfig::default().with_grace_period(Duration::from_secs(1));
        let detector = PartitionDetector::new(config);
        detector.update_expected_size(10);

        let status = detector.check_partition(4).await;
        assert!(matches!(status, PartitionStatus::Suspect { .. }));

        let status = detector.check_partition(10).await;
        assert_eq!(
            status,
            PartitionStatus::Healthy {
                current_size: 10,
                expected_size: 10
            }
        );
    }

    #[tokio::test]
    async fn test_threshold_boundary() {
        let detector = PartitionDetector::new(PartitionConfig::default());
        detector.update_expected_size(10);

        let status = detector.check_partition(5).await;
        assert_eq!(
            status,
            PartitionStatus::Healthy {
                current_size: 5,
                expected_size: 10
            }
        );

        let status = detector.check_partition(4).await;
        assert!(matches!(status, PartitionStatus::Suspect { .. }));
    }

    #[tokio::test]
    async fn test_unknown_status() {
        let detector = PartitionDetector::new(PartitionConfig::default());

        let status = detector.check_partition(5).await;
        assert_eq!(status, PartitionStatus::Unknown);
    }

    #[tokio::test]
    async fn test_grace_remaining_calculation() {
        let config = PartitionConfig::default().with_grace_period(Duration::from_secs(10));
        let detector = PartitionDetector::new(config);
        detector.update_expected_size(10);

        let status = detector.check_partition(4).await;

        match status {
            PartitionStatus::Suspect {
                grace_remaining, ..
            } => {
                assert!(grace_remaining <= Duration::from_secs(10));
                assert!(grace_remaining > Duration::from_secs(9));
            }
            _ => panic!("Expected Suspect status"),
        }
    }

    #[tokio::test]
    async fn test_config_builder() {
        let config = PartitionConfig::new()
            .with_threshold(0.6)
            .with_grace_period(Duration::from_secs(15))
            .with_read_only_on_partition(true);

        assert_eq!(config.threshold, 0.6);
        assert_eq!(config.grace_period, Duration::from_secs(15));
        assert!(config.read_only_on_partition);
    }

    #[tokio::test]
    async fn test_concurrent_checks() {
        use std::sync::Arc;
        use tokio::task;

        let config = PartitionConfig::default().with_grace_period(Duration::from_millis(100));
        let detector = Arc::new(PartitionDetector::new(config));
        detector.update_expected_size(10);

        let mut handles = vec![];
        for _ in 0..10 {
            let d = detector.clone();
            let handle = task::spawn(async move { d.check_partition(4).await });
            handles.push(handle);
        }

        for handle in handles {
            let status = handle.await.unwrap();
            assert!(matches!(status, PartitionStatus::Suspect { .. }));
        }
    }
}
