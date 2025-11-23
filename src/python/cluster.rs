//! Python bindings for RpcNet cluster functionality
//!
//! This module exposes SWIM gossip protocol, failure detection, and cluster
//! membership management to Python.

use crate::cluster::{
    ClusterConfig, ClusterEvent, ClusterEventReceiver, ClusterMembership, GossipConfig,
    HealthCheckConfig, NodeId, PoolConfig,
};
use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;
use s2n_quic::Client as QuicClient;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Python wrapper for GossipConfig
#[pyclass(name = "GossipConfig")]
#[derive(Clone)]
pub struct PyGossipConfig {
    pub(crate) inner: GossipConfig,
}

#[pymethods]
impl PyGossipConfig {
    #[new]
    #[pyo3(signature = (protocol_period_ms=1000, ack_timeout_ms=500, indirect_timeout_ms=1000, indirect_ping_count=3))]
    fn new(
        protocol_period_ms: u64,
        ack_timeout_ms: u64,
        indirect_timeout_ms: u64,
        indirect_ping_count: usize,
    ) -> Self {
        let config = GossipConfig {
            protocol_period: Duration::from_millis(protocol_period_ms),
            indirect_ping_count,
            ack_timeout: Duration::from_millis(ack_timeout_ms),
            indirect_timeout: Duration::from_millis(indirect_timeout_ms),
        };

        PyGossipConfig { inner: config }
    }

    fn __repr__(&self) -> String {
        format!(
            "GossipConfig(protocol_period={}ms, ack_timeout={}ms, indirect_timeout={}ms, indirect_ping_count={})",
            self.inner.protocol_period.as_millis(),
            self.inner.ack_timeout.as_millis(),
            self.inner.indirect_timeout.as_millis(),
            self.inner.indirect_ping_count
        )
    }
}

/// Python wrapper for HealthCheckConfig
#[pyclass(name = "HealthCheckConfig")]
#[derive(Clone)]
pub struct PyHealthCheckConfig {
    pub(crate) inner: HealthCheckConfig,
}

#[pymethods]
impl PyHealthCheckConfig {
    #[new]
    #[pyo3(signature = (check_interval_secs=5, phi_threshold=8.0))]
    fn new(check_interval_secs: u64, phi_threshold: f64) -> Self {
        PyHealthCheckConfig {
            inner: HealthCheckConfig {
                check_interval: Duration::from_secs(check_interval_secs),
                phi_threshold,
            },
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "HealthCheckConfig(check_interval={}s, phi_threshold={})",
            self.inner.check_interval.as_secs(),
            self.inner.phi_threshold
        )
    }
}

/// Python wrapper for PoolConfig
#[pyclass(name = "PoolConfig")]
#[derive(Clone)]
pub struct PyPoolConfig {
    pub(crate) inner: PoolConfig,
}

#[pymethods]
impl PyPoolConfig {
    #[new]
    #[pyo3(signature = (max_per_peer=10, max_total=100, idle_timeout_secs=300, connect_timeout_secs=10, health_check_interval_secs=60))]
    fn new(
        max_per_peer: usize,
        max_total: usize,
        idle_timeout_secs: u64,
        connect_timeout_secs: u64,
        health_check_interval_secs: u64,
    ) -> Self {
        PyPoolConfig {
            inner: PoolConfig {
                max_per_peer,
                max_total,
                idle_timeout: Duration::from_secs(idle_timeout_secs),
                connect_timeout: Duration::from_secs(connect_timeout_secs),
                health_check_interval: Duration::from_secs(health_check_interval_secs),
            },
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "PoolConfig(max_per_peer={}, max_total={}, idle_timeout={}s)",
            self.inner.max_per_peer,
            self.inner.max_total,
            self.inner.idle_timeout.as_secs()
        )
    }
}

/// Python wrapper for ClusterConfig
#[pyclass(name = "ClusterConfig")]
#[derive(Clone)]
pub struct PyClusterConfig {
    pub(crate) inner: ClusterConfig,
}

#[pymethods]
impl PyClusterConfig {
    #[new]
    #[pyo3(signature = (node_id=None, gossip=None, health=None, pool=None, bootstrap_timeout_secs=30))]
    fn new(
        node_id: Option<String>,
        gossip: Option<PyGossipConfig>,
        health: Option<PyHealthCheckConfig>,
        pool: Option<PyPoolConfig>,
        bootstrap_timeout_secs: u64,
    ) -> Self {
        PyClusterConfig {
            inner: ClusterConfig {
                node_id: node_id.map(NodeId::new),
                gossip: gossip.map(|g| g.inner).unwrap_or_default(),
                health: health.map(|h| h.inner).unwrap_or_default(),
                pool: pool.map(|p| p.inner).unwrap_or_default(),
                bootstrap_timeout: Duration::from_secs(bootstrap_timeout_secs),
            },
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ClusterConfig(node_id={:?}, bootstrap_timeout={}s)",
            self.inner.node_id.as_ref().map(|id| &id.0),
            self.inner.bootstrap_timeout.as_secs()
        )
    }
}

/// Python wrapper for QUIC Client
#[pyclass(name = "QuicClient")]
#[derive(Clone)]
pub struct PyQuicClient {
    pub(crate) inner: Arc<QuicClient>,
}

#[pymethods]
impl PyQuicClient {
    /// Create a new QUIC client
    #[staticmethod]
    #[pyo3(signature = (cert_path, bind_addr=None))]
    fn create<'py>(
        py: Python<'py>,
        cert_path: String,
        bind_addr: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        future_into_py(py, async move {
            let bind = bind_addr.unwrap_or_else(|| "0.0.0.0:0".to_string());

            let client = QuicClient::builder()
                .with_tls(Path::new(&cert_path))
                .map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                        "Failed to load TLS certificate: {}",
                        e
                    ))
                })?
                .with_io(bind.as_str())
                .map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                        "Failed to bind to {}: {}",
                        bind, e
                    ))
                })?
                .start()
                .map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                        "Failed to start QUIC client: {}",
                        e
                    ))
                })?;

            Ok(PyQuicClient {
                inner: Arc::new(client),
            })
        })
    }

    fn __repr__(&self) -> String {
        "QuicClient()".to_string()
    }
}

/// Python wrapper for cluster membership
#[pyclass(name = "Cluster")]
pub struct PyCluster {
    pub(crate) inner: Arc<ClusterMembership>,
}

#[pymethods]
impl PyCluster {
    /// Update multiple tags at once
    fn update_tags<'py>(
        &self,
        py: Python<'py>,
        tags: Vec<(String, String)>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let cluster = self.inner.clone();
        future_into_py(py, async move {
            cluster.update_tags(tags).await;
            Ok(())
        })
    }

    /// Update a single tag
    fn update_tag<'py>(
        &self,
        py: Python<'py>,
        key: String,
        value: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let cluster = self.inner.clone();
        future_into_py(py, async move {
            cluster.update_tag(key, value).await;
            Ok(())
        })
    }

    /// Subscribe to cluster events
    fn subscribe(&self) -> PyClusterEventReceiver {
        PyClusterEventReceiver {
            receiver: Arc::new(tokio::sync::Mutex::new(self.inner.subscribe())),
        }
    }

    /// Stop sending SWIM heartbeat ACKs (for testing failure scenarios)
    fn stop_heartbeats(&self) {
        self.inner.stop_heartbeats();
    }

    /// Resume sending SWIM heartbeat ACKs
    fn resume_heartbeats(&self) {
        self.inner.resume_heartbeats();
    }

    fn __repr__(&self) -> String {
        format!("Cluster(node_id={:?})", self.inner.node_id().0)
    }
}

impl PyCluster {
    pub fn new(membership: Arc<ClusterMembership>) -> Self {
        PyCluster { inner: membership }
    }
}

/// Python wrapper for cluster events
#[pyclass(name = "ClusterEventReceiver")]
pub struct PyClusterEventReceiver {
    receiver: Arc<tokio::sync::Mutex<ClusterEventReceiver>>,
}

#[pymethods]
impl PyClusterEventReceiver {
    /// Receive the next cluster event
    fn recv<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let receiver = self.receiver.clone();
        future_into_py(py, async move {
            let mut receiver_guard = receiver.lock().await;
            match receiver_guard.recv().await {
                Ok(event) => Ok(Some(event_to_python(event))),
                Err(_) => Ok(None),
            }
        })
    }

    fn __repr__(&self) -> String {
        "ClusterEventReceiver()".to_string()
    }
}

/// Convert ClusterEvent to Python-friendly format
fn event_to_python(event: ClusterEvent) -> (String, String, String) {
    match event {
        ClusterEvent::NodeJoined(node) => (
            "NodeJoined".to_string(),
            node.id.0.clone(),
            node.addr.to_string(),
        ),
        ClusterEvent::NodeLeft(node_id) => {
            ("NodeLeft".to_string(), node_id.0.clone(), String::new())
        }
        ClusterEvent::NodeFailed(node_id) => {
            ("NodeFailed".to_string(), node_id.0.clone(), String::new())
        }
        ClusterEvent::NodeRecovered(node_id) => (
            "NodeRecovered".to_string(),
            node_id.0.clone(),
            String::new(),
        ),
        ClusterEvent::NodeTagsUpdated { node_id, .. } => (
            "NodeTagsUpdated".to_string(),
            node_id.0.clone(),
            String::new(),
        ),
        ClusterEvent::PartitionDetected { .. } => (
            "PartitionDetected".to_string(),
            String::new(),
            String::new(),
        ),
        ClusterEvent::EventsDropped { count } => (
            "EventsDropped".to_string(),
            count.to_string(),
            String::new(),
        ),
    }
}
