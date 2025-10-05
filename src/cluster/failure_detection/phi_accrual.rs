use statrs::distribution::{ContinuousCDF, Normal};
use std::collections::VecDeque;
use std::time::Instant;

pub struct PhiAccrualDetector {
    history: VecDeque<f64>,
    max_samples: usize,
    threshold: f64,
    last_heartbeat: Instant,
    min_samples: usize,
}

impl PhiAccrualDetector {
    pub fn new(threshold: f64, max_samples: usize, min_samples: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_samples),
            max_samples,
            threshold,
            last_heartbeat: Instant::now(),
            min_samples,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(8.0, 100, 5)
    }

    pub fn heartbeat(&mut self) {
        let now = Instant::now();
        let interval = now.duration_since(self.last_heartbeat).as_secs_f64();

        if self.history.len() >= self.max_samples {
            self.history.pop_front();
        }

        if interval > 0.0 {
            self.history.push_back(interval);
        }

        self.last_heartbeat = now;
    }

    pub fn phi(&self) -> f64 {
        if self.history.len() < self.min_samples {
            return 0.0;
        }

        let elapsed = Instant::now()
            .duration_since(self.last_heartbeat)
            .as_secs_f64();

        let mean = self.history.iter().sum::<f64>() / self.history.len() as f64;

        let variance = self
            .history
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / self.history.len() as f64;

        if variance < 1e-10 {
            return if elapsed > mean * 2.0 {
                f64::INFINITY
            } else {
                0.0
            };
        }

        let std_dev = variance.sqrt();

        let normal = match Normal::new(mean, std_dev) {
            Ok(n) => n,
            Err(_) => {
                return if elapsed > mean * 2.0 {
                    f64::INFINITY
                } else {
                    0.0
                };
            }
        };

        let prob = 1.0 - normal.cdf(elapsed);

        -prob.max(1e-10).log10()
    }

    pub fn is_available(&self, threshold: f64) -> bool {
        self.phi() < threshold
    }

    pub fn is_available_default(&self) -> bool {
        self.is_available(self.threshold)
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.last_heartbeat = Instant::now();
    }

    pub fn last_heartbeat(&self) -> Instant {
        self.last_heartbeat
    }

    pub fn sample_count(&self) -> usize {
        self.history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_phi_increases_with_time() {
        let mut detector = PhiAccrualDetector::with_defaults();

        for _ in 0..10 {
            detector.heartbeat();
            sleep(Duration::from_millis(10));
        }

        let phi1 = detector.phi();

        sleep(Duration::from_millis(50));

        let phi2 = detector.phi();

        assert!(phi2 > phi1, "Phi should increase over time");
    }

    #[test]
    fn test_min_samples_requirement() {
        let mut detector = PhiAccrualDetector::new(8.0, 100, 5);

        for i in 0..4 {
            detector.heartbeat();
            assert_eq!(
                detector.phi(),
                0.0,
                "Phi should be 0 with {} samples (less than min)",
                i + 1
            );
            sleep(Duration::from_millis(10));
        }

        detector.heartbeat();
        sleep(Duration::from_millis(10));

        assert!(detector.phi() >= 0.0);
    }

    #[test]
    fn test_heartbeat_resets_timer() {
        let mut detector = PhiAccrualDetector::with_defaults();

        for _ in 0..10 {
            detector.heartbeat();
            sleep(Duration::from_millis(10));
        }

        sleep(Duration::from_millis(50));
        let phi_before = detector.phi();

        detector.heartbeat();
        let phi_after = detector.phi();

        assert!(
            phi_after < phi_before,
            "Phi should decrease after heartbeat"
        );
    }

    #[test]
    fn test_max_samples_window() {
        let mut detector = PhiAccrualDetector::new(8.0, 10, 5);

        for _ in 0..20 {
            detector.heartbeat();
            sleep(Duration::from_millis(5));
        }

        assert_eq!(
            detector.sample_count(),
            10,
            "Should maintain max samples window"
        );
    }

    #[test]
    fn test_clear() {
        let mut detector = PhiAccrualDetector::with_defaults();

        for _ in 0..10 {
            detector.heartbeat();
            sleep(Duration::from_millis(10));
        }

        assert!(detector.sample_count() > 0);

        detector.clear();

        assert_eq!(detector.sample_count(), 0);
        assert_eq!(detector.phi(), 0.0);
    }

    #[test]
    fn test_is_available() {
        let mut detector = PhiAccrualDetector::new(5.0, 100, 5);

        for _ in 0..10 {
            detector.heartbeat();
            sleep(Duration::from_millis(10));
        }

        assert!(detector.is_available_default());

        sleep(Duration::from_millis(200));

        if detector.phi() > 5.0 {
            assert!(!detector.is_available_default());
        }
    }

    #[test]
    fn test_normal_distribution_edge_case() {
        let mut detector = PhiAccrualDetector::new(8.0, 100, 2);

        detector.heartbeat();
        sleep(Duration::from_millis(10));
        detector.heartbeat();

        let phi = detector.phi();
        assert!(phi >= 0.0 && phi.is_finite());
    }

    #[test]
    fn test_zero_variance_handling() {
        let mut detector = PhiAccrualDetector::new(8.0, 100, 5);

        for _ in 0..5 {
            detector.heartbeat();
        }

        let phi = detector.phi();
        assert!(phi.is_finite(), "Phi should handle zero variance gracefully");
    }
}
