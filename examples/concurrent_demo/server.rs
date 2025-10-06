#![allow(dead_code)]
#![allow(unused_imports)]
//! Concurrent demo server using generated code.

#[path = "generated/concurrentdemo/mod.rs"]
mod concurrentdemo;

use concurrentdemo::server::{ConcurrentDemoHandler, ConcurrentDemoServer};
use concurrentdemo::{
    AsyncTaskRequest, AsyncTaskResponse, ComputeRequest, ComputeResponse, ConcurrentError,
    GetCounterRequest, GetCounterResponse, IncrementRequest, IncrementResponse,
};
use rpcnet::RpcConfig;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

struct MyConcurrentService {
    counter: Arc<Mutex<i64>>,
}

impl MyConcurrentService {
    fn new() -> Self {
        Self {
            counter: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl ConcurrentDemoHandler for MyConcurrentService {
    async fn compute(&self, request: ComputeRequest) -> Result<ComputeResponse, ConcurrentError> {
        if request.iterations > 10_000_000 {
            return Err(ConcurrentError::InvalidParameters);
        }

        let start = std::time::Instant::now();

        // Simulate CPU-intensive work
        let mut result = 0u64;
        for i in 0..request.iterations {
            result = result.wrapping_add(i);
        }

        let duration = start.elapsed();

        Ok(ComputeResponse {
            task_id: request.task_id,
            result,
            duration_ms: duration.as_millis() as u64,
        })
    }

    async fn async_task(
        &self,
        request: AsyncTaskRequest,
    ) -> Result<AsyncTaskResponse, ConcurrentError> {
        if request.delay_ms > 30000 {
            // Max 30 seconds
            return Err(ConcurrentError::InvalidParameters);
        }

        // Simulate async work with delay
        tokio::time::sleep(Duration::from_millis(request.delay_ms)).await;

        let completed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(AsyncTaskResponse {
            task_id: request.task_id,
            completed_at,
        })
    }

    async fn increment(
        &self,
        request: IncrementRequest,
    ) -> Result<IncrementResponse, ConcurrentError> {
        let mut counter = self.counter.lock().await;
        *counter += request.amount;
        let new_value = *counter;

        Ok(IncrementResponse { new_value })
    }

    async fn get_counter(
        &self,
        _request: GetCounterRequest,
    ) -> Result<GetCounterResponse, ConcurrentError> {
        let counter = self.counter.lock().await;
        let value = *counter;

        Ok(GetCounterResponse { value })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Concurrent Demo Server (Generated Code) ===");

    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:8083")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost");

    let service = MyConcurrentService::new();
    let server = ConcurrentDemoServer::new(service, config);

    println!("Starting concurrent demo server on port 8083...");
    println!("Server supports concurrent operations and shared state");

    server.serve().await?;
    Ok(())
}
