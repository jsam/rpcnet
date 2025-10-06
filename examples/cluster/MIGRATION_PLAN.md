# RPC Library Enhancement Plan

## 📊 Implementation Status Overview

| Task | Priority | Status | Completion |
|------|----------|--------|------------|
| 1. Default Timeout for Streaming RPCs | ⭐ P1 | ✅ **COMPLETE** | 100% |
| 2. Bidirectional Stream Helpers | ⭐ P2 | ✅ **COMPLETE** | 100% |
| 3. Simplified Handler Registration | P3 | ✅ **COMPLETE** | 100% |
| 4. Cluster Self-Registration Helper | P4 | ✅ **COMPLETE** | 100% |
| 5. Bulk Tag Updates | P5 | ✅ **COMPLETE** | 100% |

**Overall Progress: 5/5 tasks complete (100%)** 🎉

---

## Overview

This document outlines the plan to migrate resilience patterns from the cluster example into the core rpcnet library. These patterns are currently implemented as boilerplate in the example but should be first-class library features.

## Motivation

The cluster example demonstrates several critical patterns for production RPC systems:
- ✅ Timeout-based failure detection - **IMPLEMENTED**
- ✅ Cancellable bidirectional streams - **IMPLEMENTED**
- ✅ Simplified handler registration - **IMPLEMENTED**
- ✅ Cluster self-registration - **IMPLEMENTED**
- ✅ Bulk tag updates - **IMPLEMENTED**

Currently, users must reimplement these patterns manually, leading to:
- Duplicated boilerplate code
- Inconsistent error handling
- Missing timeout protection
- Complex stream lifecycle management

## Migration Tasks

### 1. Default Timeout for Streaming RPCs ⭐ **PRIORITY 1** ✅ **COMPLETE**

**Current State (client.rs:152-171):**
```rust
loop {
    let result = match timeout(Duration::from_secs(3), stream.next()).await {
        Ok(Some(r)) => r,
        Ok(None) => break,
        Err(_) => {
            // timeout - worker appears dead
            worker_failed = true;
            break;
        }
    };
    // process result...
}
```

**Target API:**
```rust
// Library enhancement
impl RpcClient {
    pub async fn call_streaming_with_timeout(
        &self,
        method: &str,
        params: Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>,
        timeout: Duration,
    ) -> Result<TimeoutStream<Vec<u8>>, RpcError>;
}

// TimeoutStream wrapper that yields Result<T, TimeoutError>
pub struct TimeoutStream<T> { /* internal */ }

impl<T> Stream for TimeoutStream<T> {
    type Item = Result<T, StreamError>;
    // yields Err(StreamError::Timeout) if no data within duration
}
```

**Migration Steps:**
1. Create `TimeoutStream` wrapper in `src/client.rs`
2. Add `call_streaming_with_timeout()` method to `RpcClient`
3. Make timeout configurable via `RpcConfig::with_default_timeout(Duration)`
4. Update codegen to generate timeout-aware client methods
5. Update cluster example to use new API
6. Add integration tests for timeout behavior

**Benefits:**
- ✅ Automatic failure detection for all streaming RPCs
- ✅ No manual timeout wrapping needed
- ✅ Consistent timeout semantics across codebase
- ✅ Prevents indefinite hangs on dead connections

---

### 2. Bidirectional Stream Helpers ⭐ **PRIORITY 2**

**Current State (client.rs:109-132):**
```rust
let (tx, rx) = tokio::sync::mpsc::channel::<InferenceRequest>(10);
let request_stream = tokio_stream::wrappers::ReceiverStream::new(rx);

let sender_task = tokio::spawn({
    let conn_id = conn_id.clone();
    async move {
        for i in 0..100 {
            let req = InferenceRequest { /* ... */ };
            if tx.send(req).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
    }
});

// Later: sender_task.abort();
```

**Target API:**
```rust
// Library enhancement
pub struct BidirectionalStream<T> {
    pub sender: mpsc::Sender<T>,
    pub stream: Pin<Box<dyn Stream<Item = T> + Send>>,
    abort_handle: AbortHandle,
}

impl<T> BidirectionalStream<T> {
    pub fn new(buffer: usize) -> Self;
    pub fn abort(&self);
}

impl<T> Drop for BidirectionalStream<T> {
    fn drop(&mut self) {
        self.abort();
    }
}
```

**Usage:**
```rust
let bidir = BidirectionalStream::<InferenceRequest>::new(10);

tokio::spawn({
    let sender = bidir.sender.clone();
    async move {
        for i in 0..100 {
            let req = InferenceRequest { /* ... */ };
            if sender.send(req).await.is_err() {
                break;
            }
        }
    }
});

// Pass stream to RPC call
let response = client.generate(bidir.stream).await?;

// Automatically aborted when bidir drops or via bidir.abort()
```

**Migration Steps:**
1. Create `BidirectionalStream` in `src/streaming.rs`
2. Implement automatic abort on drop
3. Add builder pattern for configuration (buffer size, etc.)
4. Update cluster example to use new helper
5. Add documentation and examples
6. Add tests for abort behavior

**Benefits:**
- ✅ No manual abort handle management
- ✅ Automatic cleanup on drop
- ✅ Reduces boilerplate by ~20 lines per streaming call
- ✅ Prevents resource leaks

---

### 3. Simplified Handler Registration (Codegen)

**Current State (director.rs:166-228):**
```rust
server
    .register("DirectorRegistry.get_worker", move |params: Vec<u8>| {
        let registry = get_worker_registry.clone();
        async move {
            match bincode::deserialize::<GetWorkerRequest>(&params) {
                Ok(request) => {
                    // ... business logic ...
                    Ok(bincode::serialize(&response)
                        .map_err(|e| RpcError::SerializationError(e))?)
                }
                Err(e) => Err(RpcError::SerializationError(e))
            }
        }
    })
    .await;
```

**Target Codegen Output:**
```rust
// Generated Server should handle serialization internally
impl DirectorRegistryServer {
    pub async fn register_all(&mut self) {
        let handler = self.handler.clone();
        self.rpc_server
            .register_typed::<GetWorkerRequest, GetWorkerResponse>(
                "DirectorRegistry.get_worker",
                move |req| {
                    let h = handler.clone();
                    async move { h.get_worker(req).await }
                }
            )
            .await;
    }
}
```

**New Library Method:**
```rust
impl RpcServer {
    pub async fn register_typed<Req, Resp, F, Fut>(
        &mut self,
        method: &str,
        handler: F
    ) where
        Req: DeserializeOwned,
        Resp: Serialize,
        F: Fn(Req) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Resp, E>> + Send,
        E: Into<RpcError>,
    {
        // Handles serialization/deserialization internally
    }
}
```

**Migration Steps:**
1. Add `register_typed()` method to `RpcServer` in `src/server.rs`
2. Update codegen in `rpcnet-gen` to use `register_typed()`
3. Handle serialization errors internally in the library
4. Regenerate code for cluster example
5. Remove manual serialization from example code
6. Add tests for error conversion

**Benefits:**
- ✅ Removes ~15 lines of boilerplate per handler
- ✅ Type-safe request/response handling
- ✅ Centralized serialization error handling
- ✅ Cleaner generated code

---

### 4. Cluster Self-Registration Helper

**Current State (director.rs:137-151):**
```rust
let director_node_id = cluster.node_id().clone();
let director_status = NodeStatus {
    node_id: director_node_id.clone(),
    addr,
    incarnation: Incarnation::initial(),
    state: NodeState::Alive,
    last_seen: std::time::Instant::now(),
    tags: {
        let mut tags = HashMap::new();
        tags.insert("role".to_string(), "director".to_string());
        tags
    },
};
cluster.registry().insert(director_status);
```

**Target API:**
```rust
impl Cluster {
    pub async fn register_self<I>(&self, tags: I) -> Result<(), ClusterError>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let status = NodeStatus {
            node_id: self.node_id().clone(),
            addr: self.addr(),
            incarnation: Incarnation::initial(),
            state: NodeState::Alive,
            last_seen: std::time::Instant::now(),
            tags: tags.into_iter().collect(),
        };
        self.registry().insert(status);
        Ok(())
    }
}
```

**Usage:**
```rust
cluster.register_self([
    ("role", "director"),
    ("zone", "us-west-1"),
]).await?;
```

**Migration Steps:**
1. Add `register_self()` method to `Cluster` in `src/cluster/mod.rs`
2. Support both `HashMap` and iterator inputs
3. Update cluster example director to use new method
4. Add validation for tag keys/values
5. Add documentation with common tag patterns
6. Add tests

**Benefits:**
- ✅ Reduces 13 lines to 1 line
- ✅ Eliminates manual NodeStatus construction
- ✅ Consistent registration pattern across nodes
- ✅ Less error-prone

---

### 5. Bulk Tag Updates

**Current State (worker.rs:194-196):**
```rust
cluster.update_tag("role".to_string(), "worker".to_string()).await;
cluster.update_tag("label".to_string(), worker_label.clone()).await;
```

**Target API:**
```rust
impl Cluster {
    pub async fn update_tags<I>(&self, tags: I)
    where
        I: IntoIterator<Item = (String, String)>,
    {
        for (key, value) in tags {
            self.update_tag(key, value).await;
        }
    }
}
```

**Usage:**
```rust
cluster.update_tags([
    ("role", "worker"),
    ("label", &worker_label),
    ("gpu", "true"),
]).await;
```

**Migration Steps:**
1. Add `update_tags()` method to `Cluster` in `src/cluster/mod.rs`
2. Batch gossip updates to reduce network traffic
3. Update cluster example worker to use new method
4. Add atomic batch update semantics
5. Add documentation
6. Add tests for batching behavior

**Benefits:**
- ✅ Single method call for multiple tags
- ✅ Potential for batched gossip updates
- ✅ Cleaner code
- ✅ Atomic updates

---

## Implementation Order

1. **Week 1: Streaming Timeouts (Priority 1)**
   - Implement `TimeoutStream` wrapper
   - Add `call_streaming_with_timeout()` to `RpcClient`
   - Update example to use new API
   - Write tests

2. **Week 2: Bidirectional Streams (Priority 2)**
   - Implement `BidirectionalStream` helper
   - Add automatic cleanup
   - Update example to use new helper
   - Write tests

3. **Week 3: Handler Registration**
   - Add `register_typed()` to `RpcServer`
   - Update codegen in `rpcnet-gen`
   - Regenerate and test cluster example

4. **Week 4: Cluster Helpers**
   - Implement `register_self()` and `update_tags()`
   - Update cluster example
   - Write tests and documentation

## Success Criteria

- ✅ All cluster example tests pass with new APIs
- ✅ Code reduction in example (target: -150 lines)
- ✅ No breaking changes to existing public APIs
- ✅ Full test coverage for new features
- ✅ Documentation with usage examples
- ✅ Migration guide for existing users

## Breaking Changes

**None** - All enhancements are additive. Existing APIs remain unchanged.

## Documentation Updates

1. Add "Streaming Best Practices" guide
2. Add "Cluster Setup" guide with new helpers
3. Update cluster example README
4. Add API documentation for new methods
5. Create migration examples

## Testing Strategy

1. **Unit Tests**
   - TimeoutStream timeout behavior
   - BidirectionalStream abort handling
   - Tag update batching

2. **Integration Tests**
   - End-to-end streaming with timeouts
   - Worker failure detection
   - Cluster registration patterns

3. **Example Tests**
   - Cluster example still works
   - Code reduction verified
   - Performance unchanged

## Backwards Compatibility

All changes are additive. The old manual patterns will continue to work, but the new APIs provide better ergonomics and safety.

## Future Enhancements (Out of Scope)

- Retry policies for failed streams
- Circuit breaker pattern
- Request hedging
- Automatic connection pooling
- Distributed tracing integration

## Questions & Decisions

### Q1: Should timeouts be mandatory or optional?
**Decision:** Optional with configurable default. Use `RpcConfig::with_default_timeout(Duration)` to set per-client default, or use `call_streaming_with_timeout()` for per-call override.

### Q2: Should BidirectionalStream be part of core or separate crate?
**Decision:** Core library - it's fundamental to streaming RPC patterns.

### Q3: How to handle timeout vs network errors?
**Decision:** New `StreamError` enum distinguishes timeout from transport errors:
```rust
pub enum StreamError {
    Timeout,
    Transport(RpcError),
    Application(Vec<u8>), // Application-level error from handler
}
```

## Contributors

- Initial design: Based on cluster example patterns
- Implementation: TBD
- Review: TBD

---

**Last Updated:** 2025-10-05
**Status:** Planning
