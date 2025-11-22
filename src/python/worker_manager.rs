//! Worker process manager for true multi-process Python RPC server
//!
//! This module manages a pool of actual OS processes, each running a Python
//! interpreter with its own GIL. Communication happens via Unix domain sockets.

use crate::python::worker_config::WorkerConfig;
use crate::RpcError;
use pyo3::prelude::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{error, info};

/// Handle to a worker subprocess
struct WorkerHandle {
    /// Worker ID
    id: usize,
    /// Process ID
    pid: u32,
    /// Path to Unix domain socket
    socket_path: PathBuf,
    /// Child process handle
    process: Child,
    /// Unix socket connection (protected by mutex for concurrent access)
    socket: Arc<Mutex<UnixStream>>,
    /// Last heartbeat time
    #[allow(dead_code)]
    last_heartbeat: Instant,
}

/// Manages a pool of Python worker processes
pub struct WorkerManager {
    /// Worker process handles
    workers: Vec<WorkerHandle>,
    /// Next worker index for round-robin
    next_worker: AtomicUsize,
    /// Worker configuration
    #[allow(dead_code)]
    config: WorkerConfig,
    /// Registered handlers (method_name -> Python callable)
    pub handlers: Arc<Mutex<HashMap<String, PyObject>>>,
}

impl WorkerManager {
    /// Create a new worker manager and spawn worker processes
    pub fn new(config: WorkerConfig) -> Result<Self, String> {
        info!(
            "Creating WorkerManager with {} processes",
            config.num_processes
        );

        let mut workers = Vec::with_capacity(config.num_processes);

        for i in 0..config.num_processes {
            match Self::spawn_worker(i) {
                Ok(worker) => {
                    info!("Spawned worker #{} with PID {}", i, worker.pid);
                    workers.push(worker);
                }
                Err(e) => {
                    error!("Failed to spawn worker #{}: {}", i, e);
                    // Cleanup already spawned workers
                    for mut worker in workers {
                        let _ = worker.process.kill();
                    }
                    return Err(e);
                }
            }
        }

        Ok(Self {
            workers,
            next_worker: AtomicUsize::new(0),
            config,
            handlers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Spawn a single worker subprocess
    fn spawn_worker(id: usize) -> Result<WorkerHandle, String> {
        // Create unique socket path
        let socket_path = format!("/tmp/rpcnet_worker_{}_{}.sock", std::process::id(), id);

        // Remove socket file if it exists
        let _ = std::fs::remove_file(&socket_path);

        info!("Spawning worker #{} with socket: {}", id, socket_path);

        // Find Python interpreter
        let python_exe = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_string());

        // Spawn Python worker subprocess
        let process = Command::new(&python_exe)
            .arg("-m")
            .arg("rpcnet.worker")
            .arg(&socket_path)
            .arg(id.to_string())
            .spawn()
            .map_err(|e| format!("Failed to spawn Python process: {}", e))?;

        let pid = process.id();

        // Wait for worker to create socket (max 5 seconds)
        let start = Instant::now();
        while !PathBuf::from(&socket_path).exists() {
            if start.elapsed() > Duration::from_secs(5) {
                return Err("Worker socket not created within 5 seconds".to_string());
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        // Connect to worker's Unix socket
        let socket = UnixStream::connect(&socket_path)
            .map_err(|e| format!("Failed to connect to worker socket: {}", e))?;

        // Set socket timeouts
        socket
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| format!("Failed to set read timeout: {}", e))?;
        socket
            .set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| format!("Failed to set write timeout: {}", e))?;

        Ok(WorkerHandle {
            id,
            pid,
            socket_path: PathBuf::from(socket_path),
            process,
            socket: Arc::new(Mutex::new(socket)),
            last_heartbeat: Instant::now(),
        })
    }

    /// Register a handler to be sent to workers
    pub async fn register_handler(&self, method_name: String, handler: PyObject) {
        let mut handlers = self.handlers.lock().await;
        info!(
            "WorkerManager: Registering handler for method '{}'",
            method_name
        );
        handlers.insert(method_name, handler);
    }

    /// Initialize all workers with registered handlers
    pub async fn initialize_workers(&mut self) -> Result<(), String> {
        info!("Initializing workers with handlers...");

        let handlers = self.handlers.lock().await;

        // Serialize handlers using source code extraction
        let serialized_handlers = Python::with_gil(|py| -> Result<Vec<u8>, String> {
            use pyo3::types::PyDict;

            info!("Starting handler serialization...");

            // Import required modules
            let inspect = py
                .import("inspect")
                .map_err(|e| format!("Failed to import inspect: {}", e))?;
            let textwrap = py
                .import("textwrap")
                .map_err(|e| format!("Failed to import textwrap: {}", e))?;
            let json = py
                .import("json")
                .map_err(|e| format!("Failed to import json: {}", e))?;

            // Try to import cloudpickle (optional for handler instance pickling)
            let cloudpickle = py.import("cloudpickle").ok();
            let base64 = py.import("base64").ok();

            info!(
                "cloudpickle available: {}, base64 available: {}",
                cloudpickle.is_some(),
                base64.is_some()
            );

            // DEBUG: Print to Python stderr
            let sys = py.import("sys").unwrap();
            let stderr = sys.getattr("stderr").unwrap();
            let _ = stderr.call_method1(
                "write",
                (format!(
                    "RUST DEBUG: cloudpickle={}, base64={}\n",
                    cloudpickle.is_some(),
                    base64.is_some()
                ),),
            );

            // Create a dict with method name -> (source, pickled_handler)
            let handlers_dict = PyDict::new(py);

            // Try to extract and pickle the handler instance from the first closure
            let mut pickled_handler_b64: Option<String> = None;

            for (method, handler) in handlers.iter() {
                let _ = stderr.call_method1(
                    "write",
                    (format!("RUST DEBUG: Processing method {}\n", method),),
                );

                // Get the closure's `self.handler` object if it exists (only need to do this once)
                if pickled_handler_b64.is_none() && cloudpickle.is_some() && base64.is_some() {
                    let _ = stderr.call_method1(
                        "write",
                        (format!(
                            "RUST DEBUG: Attempting closure extraction for {}\n",
                            method
                        ),),
                    );
                    info!(
                        "Attempting to extract handler from closure for method: {}",
                        method
                    );
                    match handler.getattr(py, "__closure__") {
                        Ok(closure) if !closure.is_none(py) => {
                            let _ = stderr.call_method1(
                                "write",
                                ("RUST DEBUG: Found closure!\n".to_string(),),
                            );
                            info!("Found closure for method: {}", method);
                            // Downcast to tuple
                            match closure.downcast_bound::<pyo3::types::PyTuple>(py) {
                                Ok(closure_tuple) if closure_tuple.len() > 0 => {
                                    let _ = stderr.call_method1(
                                        "write",
                                        (format!(
                                            "RUST DEBUG: Closure has {} cells\n",
                                            closure_tuple.len()
                                        ),),
                                    );
                                    info!("Closure has {} cells", closure_tuple.len());
                                    match closure_tuple.get_item(0) {
                                        Ok(cell) => {
                                            let _ = stderr.call_method1(
                                                "write",
                                                ("RUST DEBUG: Got cell\n".to_string(),),
                                            );
                                            match cell.getattr("cell_contents") {
                                                Ok(handler_obj) => {
                                                    let _ = stderr.call_method1("write", ("RUST DEBUG: Got handler object, pickling...\n".to_string(),));
                                                    info!("Extracted handler object, attempting to pickle...");
                                                    // Pickle the handler instance
                                                    if let Some(ref cp) = cloudpickle {
                                                        if let Some(ref b64) = base64 {
                                                            match cp.getattr("dumps") {
                                                                Ok(dumps) => {
                                                                    let _ = stderr.call_method1("write", ("RUST DEBUG: Calling dumps...\n".to_string(),));
                                                                    match dumps
                                                                        .call1((handler_obj,))
                                                                    {
                                                                        Ok(pickled) => {
                                                                            let _ = stderr.call_method1("write", ("RUST DEBUG: Pickled!\n".to_string(),));
                                                                            match b64.getattr("b64encode") {
                                                                                Ok(b64encode) => {
                                                                                    match b64encode.call1((pickled,)) {
                                                                                        Ok(encoded) => {
                                                                                            match encoded.call_method0("decode") {
                                                                                                Ok(encoded_str) => {
                                                                                                    match encoded_str.extract::<String>() {
                                                                                                        Ok(s) => {
                                                                                                            let _ = stderr.call_method1("write", (format!("RUST DEBUG: Successfully pickled handler! Length: {}\n", s.len()),));
                                                                                                            info!("Successfully pickled handler!");
                                                                                                            pickled_handler_b64 = Some(s);
                                                                                                        }
                                                                                                        Err(e) => {
                                                                                                            let _ = stderr.call_method1("write", (format!("RUST DEBUG: Failed extract: {}\n", e),));
                                                                                                            error!("Failed to extract string: {}", e);
                                                                                                        }
                                                                                                    }
                                                                                                }
                                                                                                Err(e) => error!("Failed to decode: {}", e),
                                                                                            }
                                                                                        }
                                                                                        Err(e) => error!("Failed to encode: {}", e),
                                                                                    }
                                                                                }
                                                                                Err(e) => error!("Failed to get b64encode: {}", e),
                                                                            }
                                                                        }
                                                                        Err(e) => {
                                                                            let _ = stderr.call_method1("write", (format!("RUST DEBUG: Failed to pickle: {}\n", e),));
                                                                            error!("Failed to pickle: {}", e);
                                                                        }
                                                                    }
                                                                }
                                                                Err(e) => error!(
                                                                    "Failed to get dumps: {}",
                                                                    e
                                                                ),
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    error!("Failed to get cell_contents: {}", e)
                                                }
                                            }
                                        }
                                        Err(e) => error!("Failed to get cell: {}", e),
                                    }
                                }
                                Ok(_) => info!("Closure tuple is empty"),
                                Err(e) => error!("Failed to downcast closure to tuple: {}", e),
                            }
                        }
                        Ok(_) => info!("Handler has no closure"),
                        Err(e) => error!("Failed to get __closure__: {}", e),
                    }
                }

                // Get the source code of the handler function
                let getsource = inspect
                    .getattr("getsource")
                    .map_err(|e| format!("Failed to get getsource: {}", e))?;

                let source = getsource
                    .call1((handler,))
                    .map_err(|e| format!("Failed to get source for {}: {}", method, e))?;

                // Dedent the source code to remove class indentation
                let dedent = textwrap
                    .getattr("dedent")
                    .map_err(|e| format!("Failed to get textwrap.dedent: {}", e))?;

                let dedented_source = dedent
                    .call1((source,))
                    .map_err(|e| format!("Failed to dedent source for {}: {}", method, e))?;

                let source_str: String = dedented_source
                    .extract()
                    .map_err(|e| format!("Failed to extract source string: {}", e))?;

                // Store both source and handler pickle
                let method_data = PyDict::new(py);
                method_data
                    .set_item("source", source_str)
                    .map_err(|e| format!("Failed to set source: {}", e))?;
                if let Some(ref handler_b64) = pickled_handler_b64 {
                    method_data
                        .set_item("handler", handler_b64.clone())
                        .map_err(|e| format!("Failed to set handler: {}", e))?;
                }

                handlers_dict
                    .set_item(method, method_data)
                    .map_err(|e| format!("Failed to set item: {}", e))?;
            }

            // Convert to JSON bytes
            let dumps = json
                .getattr("dumps")
                .map_err(|e| format!("Failed to get json.dumps: {}", e))?;
            let json_str = dumps
                .call1((handlers_dict,))
                .map_err(|e| format!("Failed to serialize to JSON: {}", e))?;
            let json_bytes: String = json_str
                .extract()
                .map_err(|e| format!("Failed to extract JSON string: {}", e))?;

            Ok(json_bytes.into_bytes())
        })?;

        drop(handlers);

        // Send to each worker
        for worker in &mut self.workers {
            info!(
                "Sending handlers to worker #{} (PID {})",
                worker.id, worker.pid
            );
            let mut socket = worker.socket.lock().await;
            Self::send_init_message(&mut socket, &serialized_handlers)
                .map_err(|e| format!("Failed to initialize worker #{}: {}", worker.id, e))?;
        }

        info!("All workers initialized successfully");
        Ok(())
    }

    /// Send initialization message to worker
    fn send_init_message(socket: &mut UnixStream, data: &[u8]) -> Result<(), String> {
        // Message format: [0xFF 0xFF] [length: u32] [data]
        let header = [0xFF, 0xFF];
        socket
            .write_all(&header)
            .map_err(|e| format!("Failed to write header: {}", e))?;

        let length = (data.len() as u32).to_be_bytes();
        socket
            .write_all(&length)
            .map_err(|e| format!("Failed to write length: {}", e))?;

        socket
            .write_all(data)
            .map_err(|e| format!("Failed to write data: {}", e))?;

        socket
            .flush()
            .map_err(|e| format!("Failed to flush: {}", e))?;

        // Read acknowledgment
        let mut ack = [0u8; 1];
        socket
            .read_exact(&mut ack)
            .map_err(|e| format!("Failed to read ack: {}", e))?;

        if ack[0] != 0 {
            return Err("Worker returned error on initialization".to_string());
        }

        Ok(())
    }

    /// Execute a handler on a worker process
    pub async fn execute_handler(
        &self,
        method_name: &str,
        params: Vec<u8>,
    ) -> Result<Vec<u8>, RpcError> {
        // Round-robin worker selection
        let idx = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        let worker = &self.workers[idx];

        // Lock the socket and send request
        let socket = worker.socket.clone();
        Self::send_request(socket, method_name, &params)
            .await
            .map_err(|e| RpcError::InternalError(format!("Worker communication failed: {}", e)))
    }

    /// Send request to worker and receive response
    async fn send_request(
        socket: Arc<Mutex<UnixStream>>,
        method_name: &str,
        params: &[u8],
    ) -> Result<Vec<u8>, String> {
        let method_bytes = method_name.as_bytes();

        // Message format: [method_name_len: u16] [method_name] [params_len: u32] [params]
        let method_len = (method_bytes.len() as u16).to_be_bytes();
        let params_len = (params.len() as u32).to_be_bytes();

        let mut socket = socket.lock().await;

        socket
            .write_all(&method_len)
            .map_err(|e| format!("Failed to write method length: {}", e))?;

        socket
            .write_all(method_bytes)
            .map_err(|e| format!("Failed to write method name: {}", e))?;

        socket
            .write_all(&params_len)
            .map_err(|e| format!("Failed to write params length: {}", e))?;

        socket
            .write_all(params)
            .map_err(|e| format!("Failed to write params: {}", e))?;

        socket
            .flush()
            .map_err(|e| format!("Failed to flush: {}", e))?;

        // Read response
        // Format: [status: u8] [length: u32] [data]
        let mut status = [0u8; 1];
        socket
            .read_exact(&mut status)
            .map_err(|e| format!("Failed to read status: {}", e))?;

        let mut len_buf = [0u8; 4];
        socket
            .read_exact(&mut len_buf)
            .map_err(|e| format!("Failed to read length: {}", e))?;
        let length = u32::from_be_bytes(len_buf) as usize;

        let mut result = vec![0u8; length];
        socket
            .read_exact(&mut result)
            .map_err(|e| format!("Failed to read result: {}", e))?;

        if status[0] == 0 {
            Ok(result)
        } else {
            Err(String::from_utf8_lossy(&result).to_string())
        }
    }

    /// Shutdown all workers gracefully
    pub fn shutdown(&mut self) {
        info!("Shutting down {} workers", self.workers.len());

        for worker in &mut self.workers {
            info!("Stopping worker #{} (PID {})", worker.id, worker.pid);

            // Try graceful shutdown first
            let _ = worker.process.kill();

            // Remove socket file
            let _ = std::fs::remove_file(&worker.socket_path);
        }

        info!("All workers shut down");
    }
}

impl Drop for WorkerManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}
