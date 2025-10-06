# Production Deployment

This guide covers best practices for deploying RpcNet clusters in production environments, including security, monitoring, high availability, and operational procedures.

## Architecture Patterns

### 1. Basic Production Setup

Minimum viable production deployment:

```
                    Load Balancer (L4)
                           |
              ┌────────────┼────────────┐
              │            │            │
         ┌────▼───┐   ┌────▼───┐   ┌────▼───┐
         │Director│   │Director│   │Director│  (3+ for HA)
         │  (HA)  │   │  (HA)  │   │  (HA)  │
         └────┬───┘   └────┬───┘   └────┬───┘
              │            │            │
      ┌───────┴────────────┴────────────┴───────┐
      │                                          │
  ┌───▼────┐  ┌────────┐  ┌────────┐  ┌────────▼┐
  │Worker 1│  │Worker 2│  │Worker 3│  │Worker N │
  └────────┘  └────────┘  └────────┘  └─────────┘
```

**Components**:
- **Load Balancer**: Routes clients to healthy directors
- **Directors (3+)**: Coordinator nodes in HA configuration
- **Workers (N)**: Processing nodes, scale horizontally

### 2. Multi-Region Setup

For global deployments:

```
        Region US-EAST              Region EU-WEST
┌──────────────────────────┐  ┌──────────────────────────┐
│   Director Cluster (3)   │  │   Director Cluster (3)   │
│   Worker Pool (10+)      │  │   Worker Pool (10+)      │
└──────────┬───────────────┘  └───────────┬──────────────┘
           │                               │
           └───────────┬───────────────────┘
                       │
                 Cross-region
                 Gossip Protocol
                 (optional coordination)
```

**Benefits**:
- Lower latency for regional clients
- Fault isolation (region failure doesn't affect others)
- Regulatory compliance (data locality)

### 3. Hybrid Edge Deployment

For edge computing scenarios:

```
              Cloud (Central)
         ┌─────────────────────┐
         │  Director Cluster   │
         │  Worker Pool        │
         └──────────┬──────────┘
                    │
         ┌──────────┼──────────┐
         │          │          │
    ┌────▼───┐ ┌───▼────┐ ┌───▼────┐
    │ Edge 1 │ │ Edge 2 │ │ Edge 3 │
    │Workers │ │Workers │ │Workers │
    └────────┘ └────────┘ └────────┘
```

**Use cases**:
- IoT workloads
- Low-latency requirements
- Bandwidth optimization

## Security

### TLS Configuration

#### Production Certificates

```rust
// ❌ Bad: Self-signed certificates
let cert = std::fs::read("self_signed.pem")?;

// ✅ Good: Proper CA-signed certificates
let cert = std::fs::read("/etc/rpcnet/certs/server.crt")?;
let key = std::fs::read("/etc/rpcnet/certs/server.key")?;
let ca = std::fs::read("/etc/rpcnet/certs/ca.crt")?;

let config = ServerConfig::builder()
    .with_cert_and_key(cert, key)?
    .with_ca_cert(ca)?  // Verify clients
    .build();
```

#### Certificate Rotation

```rust
use tokio::time::{interval, Duration};

async fn rotate_certificates(server: Arc<Server>) {
    let mut check_interval = interval(Duration::from_secs(3600)); // Check hourly
    
    loop {
        check_interval.tick().await;
        
        // Check certificate expiry
        if certificate_expires_soon("/etc/rpcnet/certs/server.crt", 30).await? {
            log::warn!("Certificate expiring soon, rotating...");
            
            // Load new certificate
            let new_cert = std::fs::read("/etc/rpcnet/certs/server.crt.new")?;
            let new_key = std::fs::read("/etc/rpcnet/certs/server.key.new")?;
            
            // Hot-reload without downtime
            server.reload_certificate(new_cert, new_key).await?;
            
            log::info!("Certificate rotated successfully");
        }
    }
}
```

### Authentication & Authorization

```rust
#[rpc_trait]
pub trait SecureService {
    async fn process(&self, auth_token: String, data: Vec<u8>) -> Result<Response>;
}

#[rpc_impl]
impl SecureService for Handler {
    async fn process(&self, auth_token: String, data: Vec<u8>) -> Result<Response> {
        // Verify token
        let claims = verify_jwt(&auth_token)?;
        
        // Check permissions
        if !claims.has_permission("compute:execute") {
            return Err(anyhow::anyhow!("Insufficient permissions"));
        }
        
        // Process request
        Ok(self.do_process(data).await?)
    }
}
```

### Network Segmentation

```
┌─────────────────────────────────────────────────────┐
│                 Public Network                       │
│  (Clients, Load Balancer)                           │
└────────────────────┬────────────────────────────────┘
                     │ Firewall
┌────────────────────▼────────────────────────────────┐
│             Management Network                       │
│  (Directors, Monitoring, Logging)                   │
└────────────────────┬────────────────────────────────┘
                     │ Firewall
┌────────────────────▼────────────────────────────────┐
│              Worker Network                          │
│  (Workers, Internal Communication)                  │
└─────────────────────────────────────────────────────┘
```

**Firewall Rules**:
```bash
# Public → Management: Only load balancer ports
iptables -A FORWARD -i public -o management -p tcp --dport 8080 -j ACCEPT

# Management → Workers: Full access
iptables -A FORWARD -i management -o workers -j ACCEPT

# Workers → Workers: Gossip protocol
iptables -A FORWARD -i workers -o workers -p udp --dport 7946 -j ACCEPT
```

## Monitoring

### Essential Metrics

```rust
use prometheus::{register_gauge, register_counter, register_histogram};

// Throughput
let request_counter = register_counter!("rpc_requests_total", "Total RPC requests");
request_counter.inc();

// Latency
let latency_histogram = register_histogram!(
    "rpc_latency_seconds",
    "RPC latency distribution",
    vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
);
latency_histogram.observe(duration.as_secs_f64());

// Health
let healthy_workers = register_gauge!("cluster_healthy_workers", "Number of healthy workers");
healthy_workers.set(registry.healthy_count().await as f64);

// Errors
let error_counter = register_counter!("rpc_errors_total", "Total RPC errors", &["type"]);
error_counter.with_label_values(&["timeout"]).inc();
```

### Prometheus Integration

```rust
use prometheus::{Encoder, TextEncoder};
use warp::Filter;

async fn start_metrics_server() {
    let metrics_route = warp::path!("metrics").map(|| {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = vec![];
        encoder.encode(&metric_families, &mut buffer).unwrap();
        
        warp::reply::with_header(
            buffer,
            "Content-Type",
            "text/plain; charset=utf-8",
        )
    });
    
    warp::serve(metrics_route)
        .run(([0, 0, 0, 0], 9090))
        .await;
}
```

**Prometheus config** (`prometheus.yml`):
```yaml
scrape_configs:
  - job_name: 'rpcnet_directors'
    static_configs:
      - targets: ['director-1:9090', 'director-2:9090', 'director-3:9090']
  
  - job_name: 'rpcnet_workers'
    static_configs:
      - targets: ['worker-1:9090', 'worker-2:9090', 'worker-3:9090']
```

### Grafana Dashboards

**Key panels**:

1. **Throughput**: `rate(rpc_requests_total[1m])`
2. **Latency P99**: `histogram_quantile(0.99, rpc_latency_seconds)`
3. **Error Rate**: `rate(rpc_errors_total[1m])`
4. **Worker Health**: `cluster_healthy_workers`

### Alerting

```yaml
# alerts.yml
groups:
  - name: rpcnet
    interval: 30s
    rules:
      - alert: HighErrorRate
        expr: rate(rpc_errors_total[5m]) > 0.05
        for: 2m
        annotations:
          summary: "High RPC error rate detected"
      
      - alert: LowWorkerCount
        expr: cluster_healthy_workers < 3
        for: 1m
        annotations:
          summary: "Less than 3 healthy workers available"
      
      - alert: HighLatency
        expr: histogram_quantile(0.99, rpc_latency_seconds) > 0.1
        for: 5m
        annotations:
          summary: "P99 latency above 100ms"
```

## Logging

### Structured Logging

```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(data))]
async fn process_request(request_id: Uuid, worker_id: Uuid, data: Vec<u8>) -> Result<Response> {
    info!(
        request_id = %request_id,
        worker_id = %worker_id,
        data_size = data.len(),
        "Processing request"
    );
    
    match worker.call("compute", data).await {
        Ok(response) => {
            info!(
                request_id = %request_id,
                worker_id = %worker_id,
                response_size = response.len(),
                "Request completed"
            );
            Ok(response)
        }
        Err(e) => {
            error!(
                request_id = %request_id,
                worker_id = %worker_id,
                error = %e,
                "Request failed"
            );
            Err(e)
        }
    }
}
```

### Log Aggregation

**Fluentd config** (`fluent.conf`):
```
<source>
  @type forward
  port 24224
</source>

<match rpcnet.**>
  @type elasticsearch
  host elasticsearch.example.com
  port 9200
  index_name rpcnet
  type_name logs
</match>
```

## High Availability

### Director HA Setup

```rust
// Each director is identical, configured via environment
let director_id = Uuid::new_v4();
let cluster_config = ClusterConfig::default()
    .with_bind_addr(env::var("BIND_ADDR")?.parse()?)
    .with_seeds(parse_seeds(&env::var("SEED_NODES")?)?);

let cluster = server.enable_cluster(cluster_config).await?;

// Tag as director
cluster.set_tag("role", "director");
cluster.set_tag("id", &director_id.to_string());

// All directors operate identically, clients can use any one
```

### Graceful Shutdown

```rust
use tokio::signal;

async fn run_server(mut server: Server) -> Result<()> {
    // Spawn server task
    let server_handle = tokio::spawn(async move {
        server.run().await
    });
    
    // Wait for shutdown signal
    signal::ctrl_c().await?;
    
    log::info!("Shutdown signal received, gracefully shutting down...");
    
    // 1. Stop accepting new connections
    server.stop_accepting().await;
    
    // 2. Wait for in-flight requests (with timeout)
    tokio::time::timeout(
        Duration::from_secs(30),
        server.wait_for_in_flight()
    ).await?;
    
    // 3. Leave cluster gracefully
    cluster.leave().await?;
    
    // 4. Close connections
    server.shutdown().await?;
    
    log::info!("Shutdown complete");
    Ok(())
}
```

### Health Checks

```rust
#[rpc_trait]
pub trait HealthService {
    async fn health(&self) -> Result<HealthStatus>;
    async fn ready(&self) -> Result<ReadyStatus>;
}

#[derive(Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub version: String,
    pub uptime_secs: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ReadyStatus {
    pub ready: bool,
    pub workers_available: usize,
    pub cluster_size: usize,
}

#[rpc_impl]
impl HealthService for Handler {
    async fn health(&self) -> Result<HealthStatus> {
        Ok(HealthStatus {
            healthy: true,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: self.start_time.elapsed().as_secs(),
        })
    }
    
    async fn ready(&self) -> Result<ReadyStatus> {
        let workers = self.registry.worker_count().await;
        let cluster_size = self.cluster.node_count().await;
        
        Ok(ReadyStatus {
            ready: workers > 0,
            workers_available: workers,
            cluster_size,
        })
    }
}
```

**Kubernetes probes**:
```yaml
livenessProbe:
  exec:
    command:
    - /usr/local/bin/health-check
    - --endpoint=health
  initialDelaySeconds: 10
  periodSeconds: 10

readinessProbe:
  exec:
    command:
    - /usr/local/bin/health-check
    - --endpoint=ready
  initialDelaySeconds: 5
  periodSeconds: 5
```

## Deployment

### Docker

**Dockerfile**:
```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/director /usr/local/bin/
COPY --from=builder /app/target/release/worker /usr/local/bin/

# Expose ports
EXPOSE 8080 7946/udp

CMD ["director"]
```

**Docker Compose** (`docker-compose.yml`):
```yaml
version: '3.8'

services:
  director-1:
    image: rpcnet:latest
    command: director
    environment:
      - DIRECTOR_ADDR=0.0.0.0:8080
      - RUST_LOG=info
    ports:
      - "8080:8080"
      - "7946:7946/udp"
  
  worker-1:
    image: rpcnet:latest
    command: worker
    environment:
      - WORKER_LABEL=worker-1
      - WORKER_ADDR=0.0.0.0:8081
      - DIRECTOR_ADDR=director-1:8080
      - RUST_LOG=info
    depends_on:
      - director-1
```

### Kubernetes

**Deployment** (`director-deployment.yaml`):
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rpcnet-director
spec:
  replicas: 3
  selector:
    matchLabels:
      app: rpcnet-director
  template:
    metadata:
      labels:
        app: rpcnet-director
    spec:
      containers:
      - name: director
        image: rpcnet:latest
        command: ["director"]
        env:
        - name: DIRECTOR_ADDR
          value: "0.0.0.0:8080"
        - name: RUST_LOG
          value: "info"
        ports:
        - containerPort: 8080
          name: rpc
        - containerPort: 7946
          name: gossip
          protocol: UDP
        resources:
          requests:
            memory: "256Mi"
            cpu: "500m"
          limits:
            memory: "512Mi"
            cpu: "1000m"
```

**Service** (`director-service.yaml`):
```yaml
apiVersion: v1
kind: Service
metadata:
  name: rpcnet-director
spec:
  type: LoadBalancer
  selector:
    app: rpcnet-director
  ports:
  - name: rpc
    port: 8080
    targetPort: 8080
  - name: gossip
    port: 7946
    targetPort: 7946
    protocol: UDP
```

**HorizontalPodAutoscaler**:
```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: rpcnet-worker-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: rpcnet-worker
  minReplicas: 3
  maxReplicas: 20
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
```

## Configuration Management

### Environment-Based Config

```rust
use config::{Config, Environment, File};

#[derive(Debug, Deserialize)]
struct Settings {
    server: ServerSettings,
    cluster: ClusterSettings,
    monitoring: MonitoringSettings,
}

#[derive(Debug, Deserialize)]
struct ServerSettings {
    bind_addr: String,
    cert_path: String,
    key_path: String,
}

fn load_config() -> Result<Settings> {
    let settings = Config::builder()
        // Default config
        .add_source(File::with_name("config/default"))
        // Environment-specific config (optional)
        .add_source(File::with_name(&format!("config/{}", env!("ENV"))).required(false))
        // Environment variables (override)
        .add_source(Environment::with_prefix("RPCNET"))
        .build()?;
    
    settings.try_deserialize()
}
```

### Secret Management

```rust
use aws_sdk_secretsmanager::Client as SecretsClient;

async fn load_tls_certs_from_secrets() -> Result<(Vec<u8>, Vec<u8>)> {
    let config = aws_config::load_from_env().await;
    let client = SecretsClient::new(&config);
    
    // Load certificate
    let cert_secret = client
        .get_secret_value()
        .secret_id("rpcnet/production/tls_cert")
        .send()
        .await?;
    let cert = cert_secret.secret_binary().unwrap().as_ref().to_vec();
    
    // Load key
    let key_secret = client
        .get_secret_value()
        .secret_id("rpcnet/production/tls_key")
        .send()
        .await?;
    let key = key_secret.secret_binary().unwrap().as_ref().to_vec();
    
    Ok((cert, key))
}
```

## Operational Procedures

### Rolling Updates

```bash
#!/bin/bash
# Rolling update script for workers

WORKERS=("worker-1" "worker-2" "worker-3" "worker-4")

for worker in "${WORKERS[@]}"; do
    echo "Updating $worker..."
    
    # Gracefully shutdown worker
    kubectl exec $worker -- kill -SIGTERM 1
    
    # Wait for worker to leave cluster
    sleep 10
    
    # Update image
    kubectl set image deployment/rpcnet-worker worker=rpcnet:new-version
    
    # Wait for new pod to be ready
    kubectl wait --for=condition=ready pod -l app=$worker --timeout=60s
    
    # Verify worker joined cluster
    kubectl exec director-1 -- check-worker-registered $worker
    
    echo "$worker updated successfully"
done
```

### Backup and Restore

```rust
// Backup cluster state (metadata only, not data)
async fn backup_cluster_state(cluster: Arc<ClusterMembership>) -> Result<()> {
    let state = ClusterState {
        nodes: cluster.nodes().await,
        timestamp: SystemTime::now(),
    };
    
    let backup = serde_json::to_vec(&state)?;
    std::fs::write("/backup/cluster_state.json", backup)?;
    
    Ok(())
}

// Restore from backup (for disaster recovery)
async fn restore_cluster_state(path: &str) -> Result<ClusterState> {
    let backup = std::fs::read(path)?;
    let state: ClusterState = serde_json::from_slice(&backup)?;
    Ok(state)
}
```

### Runbooks

**Worker Node Failure**:
1. Verify failure: `kubectl get pods | grep worker`
2. Check logs: `kubectl logs <worker-pod>`
3. If recoverable: `kubectl delete pod <worker-pod>` (auto-restarts)
4. If not: Investigate root cause, fix, redeploy
5. Verify cluster health: `kubectl exec director-1 -- cluster-health`

**High Latency**:
1. Check Grafana: Identify which nodes have high latency
2. SSH to affected nodes: `ssh worker-5`
3. Check CPU/memory: `top`, `free -h`
4. Check network: `netstat -s`, `iftop`
5. Review logs: `journalctl -u rpcnet-worker -n 1000`
6. If needed: Scale up workers or restart affected nodes

## Cost Optimization

### Resource Sizing

```rust
// Right-size based on actual usage
async fn recommend_sizing(metrics: &Metrics) -> Recommendation {
    let avg_cpu = metrics.avg_cpu_usage();
    let avg_memory = metrics.avg_memory_usage();
    let p99_cpu = metrics.p99_cpu_usage();
    
    if avg_cpu < 30.0 && p99_cpu < 60.0 {
        Recommendation::DownsizeWorkers
    } else if p99_cpu > 80.0 {
        Recommendation::UpsizeWorkers
    } else {
        Recommendation::CurrentSizingOptimal
    }
}
```

### Auto-Scaling

```yaml
# Scale workers based on request rate
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: rpcnet-worker-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: rpcnet-worker
  minReplicas: 2
  maxReplicas: 20
  metrics:
  - type: Pods
    pods:
      metric:
        name: rpc_requests_per_second
      target:
        type: AverageValue
        averageValue: "5000"  # Scale when > 5K RPS per worker
```

## Checklist

### Pre-Deployment

- [ ] TLS certificates from trusted CA
- [ ] Secrets stored in secret manager (not env vars)
- [ ] Monitoring and alerting configured
- [ ] Log aggregation set up
- [ ] Health checks implemented
- [ ] Graceful shutdown handling
- [ ] Resource limits configured
- [ ] Auto-scaling rules defined
- [ ] Backup procedures tested
- [ ] Runbooks documented

### Post-Deployment

- [ ] Verify all nodes healthy
- [ ] Check metrics dashboards
- [ ] Test failover scenarios
- [ ] Validate performance (latency, throughput)
- [ ] Review logs for errors
- [ ] Test rolling updates
- [ ] Verify backups working
- [ ] Update documentation

## Next Steps

- **[Performance Tuning](performance.md)** - Optimize for production load
- **[Failure Handling](../cluster/failures.md)** - Handle production incidents
- **[Migration Guide](migration.md)** - Migrate existing systems

## References

- [Kubernetes Best Practices](https://kubernetes.io/docs/concepts/configuration/overview/) - K8s configuration
- [Prometheus Monitoring](https://prometheus.io/docs/practices/naming/) - Metrics best practices
- [AWS Well-Architected](https://aws.amazon.com/architecture/well-architected/) - Cloud architecture patterns
