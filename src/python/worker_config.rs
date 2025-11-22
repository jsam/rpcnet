//! Worker configuration for multi-process Python RPC server
//!
//! This module provides configuration structures for worker processes
//! in the multi-process server architecture.

use std::thread;

/// Configuration for worker processes
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Number of worker processes to spawn
    /// Default: CPU count
    pub num_processes: usize,

    /// Number of worker threads per process
    /// Default: 4
    pub workers_per_process: usize,

    /// Graceful shutdown timeout in seconds
    /// Default: 30
    pub graceful_shutdown_timeout_secs: u64,

    /// Worker restart on failure
    /// Default: true
    pub auto_restart: bool,

    /// Maximum number of restart attempts per worker
    /// Default: 3
    pub max_restart_attempts: usize,
}

impl WorkerConfig {
    /// Create a new worker configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of worker processes
    pub fn with_processes(mut self, num: usize) -> Self {
        self.num_processes = num;
        self
    }

    /// Set the number of workers per process
    pub fn with_workers(mut self, num: usize) -> Self {
        self.workers_per_process = num;
        self
    }

    /// Set graceful shutdown timeout
    pub fn with_shutdown_timeout(mut self, secs: u64) -> Self {
        self.graceful_shutdown_timeout_secs = secs;
        self
    }

    /// Enable/disable auto-restart
    pub fn with_auto_restart(mut self, enabled: bool) -> Self {
        self.auto_restart = enabled;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.num_processes == 0 {
            return Err("num_processes must be at least 1".to_string());
        }

        if self.workers_per_process == 0 {
            return Err("workers_per_process must be at least 1".to_string());
        }

        if self.num_processes > 128 {
            return Err("num_processes cannot exceed 128 (too many processes)".to_string());
        }

        if self.workers_per_process > 64 {
            return Err("workers_per_process cannot exceed 64 (too many threads)".to_string());
        }

        Ok(())
    }

    /// Get the total number of execution contexts (processes × workers)
    pub fn total_execution_contexts(&self) -> usize {
        self.num_processes * self.workers_per_process
    }
}

impl Default for WorkerConfig {
    fn default() -> Self {
        // Get CPU count, defaulting to 4 if unable to detect
        let cpu_count = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        Self {
            num_processes: cpu_count,
            workers_per_process: 4,
            graceful_shutdown_timeout_secs: 30,
            auto_restart: true,
            max_restart_attempts: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WorkerConfig::default();
        assert!(config.num_processes > 0);
        assert_eq!(config.workers_per_process, 4);
        assert_eq!(config.graceful_shutdown_timeout_secs, 30);
        assert!(config.auto_restart);
    }

    #[test]
    fn test_custom_config() {
        let config = WorkerConfig::new()
            .with_processes(8)
            .with_workers(16)
            .with_shutdown_timeout(60);

        assert_eq!(config.num_processes, 8);
        assert_eq!(config.workers_per_process, 16);
        assert_eq!(config.graceful_shutdown_timeout_secs, 60);
    }

    #[test]
    fn test_validation() {
        let config = WorkerConfig::new().with_processes(0);
        assert!(config.validate().is_err());

        let config = WorkerConfig::new().with_workers(0);
        assert!(config.validate().is_err());

        let config = WorkerConfig::new().with_processes(200);
        assert!(config.validate().is_err());

        let config = WorkerConfig::new();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_total_contexts() {
        let config = WorkerConfig::new().with_processes(4).with_workers(8);

        assert_eq!(config.total_execution_contexts(), 32);
    }
}
