# RFC: Enhancing RpcNet for Machine Learning Model Serving

* Status: Draft
* Author: RpcNet Team
* Target Release: v0.3.0

## 1. Motivation

Partners plan to embed RpcNet into a model serving stack that supports both
batch/offline inference and low-latency streaming inference. The current APIs
require manual framing and handler wiring for these scenarios. We can improve
developer experience by providing presets and tooling tailored for ML workloads.

Key goals:

- Simplify handling of batched inputs (e.g., Tensor batches or serialized lists)
- Provide structured streaming flows (token-by-token output, request cancellation)
- Offer backpressure and QoS guidelines suitable for GPU-backed inference servers

## 2. Overview of Proposed Improvements

1. **Batch Context Helpers** – Extend `RpcServer` and `RpcClient` with utilities
   for batch metadata (batch size, request IDs) and automatic chunking, including
   native support for heterogeneous / variable-length tensors.
2. **Streaming Profiles** – Introduce predefined streaming modes (token stream,
   delta updates) that wrap the generic `call_streaming` API.
3. **Resource Coordination** – Document concurrency controls, GPU scheduling hints,
   and integration patterns with async model runtimes.
4. **Test Harness Extensions** – Ship stub handlers and client fixtures exercising
   batched and streaming use cases.

## 3. Detailed Design

### 3.1 Batch Context Helpers

**Server**

- Add optional `BatchContext` parameter to unary handlers via a new registration
  method: `register_batched(method, handler)`.
- `BatchContext` exposes aggregated metadata (batch size, per-item offsets) and
  helper serializers (e.g., `split_batch<T>` returning structured items). For
  heterogeneous batches we additionally surface per-item byte ranges, user tags,
  and optional padding descriptors so model code can reconstruct ragged tensors
  without copying the entire batch.
- For existing `register`, behaviour remains unchanged; the new method shares the
  same handler map but wraps the closure with batch-specific parsing.

**Client**

- Provide `call_batched(method, items, encoder)` that handles serialization of a
  sequence of inputs, chunking if necessary, and response reassembly. The client
  helper accepts heterogeneous payloads by delegating to a user-provided
  `BatchEncoder` trait; when omitted we supply a default implementation capable
  of packing ragged tensors with offset tables.
- Expose batching strategy hooks (size-based, latency-based, custom aggregator).

### 3.2 Streaming Profiles

Define two high-level streaming wrappers:

1. `call_token_stream(method, request)` – for autoregressive models; returns a
   stream of tokens (`Result<TokenChunk, RpcError>`). Tokens can carry metadata
   such as probability or timestamp.
2. `call_delta_stream(method, base_request)` – for applications like speech
   recognition where the server emits partial hypotheses. Each frame indicates
   whether it replaces or appends to the previous output.

On the server, matching handlers can use new helper types `TokenStreamResponder`
 and `DeltaStreamResponder` that provide `send_token`, `finish`, and error helpers.

### 3.3 Resource Coordination, Metrics & Cancellation

- Document best practices for GPU-bound workloads: limiting concurrent streams,
  using structured batching (e.g., micro-batches), and reacting to client-driven
  cancellation (`drop` behaviour on the stream).
- Provide optional middleware (feature-flagged) that enforces queue limits and
  returns `RpcError::StreamError("overloaded")` when limits are exceeded.
- Ship built-in Prometheus metrics exporters capturing queue depth, batch merge
  latency, GPU dispatch timing, token throughput, and cancellation events. This
  should integrate with the middleware above so operators can correlate pressure
  with application-level limits.
- Expand cancellation semantics: streaming responders gain `cancel_token()` and
  `cancel_batch_item(idx)` utilities so handlers can terminate token emission or
  drop individual batch entries. On the client we emit cancellation frames which
  the server surface propagates to user handlers via the context.

### 3.4 Testing Support

- Expand existing mock infrastructure with batched and streaming fixtures:
  - `MockBatchRequest` / `MockBatchResponse` for unit tests.
  - Streaming test clients that simulate token delays and cancellation.
- Include sample tests in `doc_examples_tests` demonstrating multi-token output
  and batch decomposition to keep coverage intact.

### 3.5 Broker-Mediated Connection Swapping

Target deployments include a broker tier that performs connection admission and
worker selection but does not host GPUs. QUIC allows migrating an existing
connection between endpoints. We propose the following mechanism:

1. **Negotiation Phase** – Client establishes a QUIC connection to the broker.
   During the initial RPC handshake the broker selects a worker and issues a
   `Connection-Handoff` control frame containing the worker's address, keying
   material (if mutual TLS is required), and a one-time transfer token.
2. **Handoff Phase** – Broker pauses application streams, forwards transport
   state to the worker (using QUIC connection migration primitives or a custom
   control channel), and acknowledges once the worker takes ownership.
3. **Post-Handoff Phase** – Client continues streaming directly with the worker;
   broker steps out of the hot path, only monitoring health via metrics.

Critical review:

- **Security** – We must ensure the worker cannot impersonate other services.
  Tokens should be bound to client identity and expire immediately after use.
  Mutual TLS re-validation may be necessary; otherwise, man-in-the-middle risks
  arise if the broker is compromised. QUIC connection migration does not change
  encryption keys automatically, so we need to rekey or wrap the handoff in an
  authenticated envelope.
- **State Synchronisation** – Streams mid-flight during handoff need buffering.
  If the broker mishandles flow-control windows the client may deadlock. We must
  model pause/resume semantics carefully, possibly requiring a short drain phase
  before migration completes. Instrumentation must highlight handoffs that take
  longer than expected.
- **Failure Scenarios** – Workers may refuse or fail after accepting handoff.
  The broker should fallback to another worker or resume handling the connection
  itself. QUIC permits multiple path migrations; we must track successive
  attempts and cap retries to avoid thrashing.
- **Observability** – Integrated metrics should record the number of active
  handoffs, latency per stage, and failure rates so operators can tune broker
  behaviour.
- **Compatibility** – Clients unaware of the feature should still function; we
  therefore negotiate the capability via a new RPC protocol version flag.

Implementation tasks include a broker library exposing handoff primitives, a
worker shim to accept migrated connections, and test harnesses that simulate
broker/worker topologies with packet loss.

## 4. Implementation Plan

1. **Milestone A (foundational helpers)**
   - Implement `BatchContext`, `register_batched`, and `call_batched`.
   - Add docbook chapter on batching.
   - Update tests with mock batches.

2. **Milestone B (streaming profiles)**
   - Implement token/delta streaming responders and client wrappers.
   - Provide example server handlers for autoregressive decoding.
   - Extend docbook streaming chapter with ML-focused guidance and cancellation
     semantics.

3. **Milestone C (resource coordination)**
   - Add optional middleware for queue limits and cancellation handling.
   - Integrate Prometheus metrics exporters covering batching, streaming, and
     handoff health.
   - Document tuning suggestions for GPU inference.

4. **Milestone D (final polish)**
   - Comprehensive documentation refresh.
   - Internal QA with integration tests mimicking model-serving workloads.
   - Broker handoff soak tests and failover drills.

## 5. Alternatives Considered

- **Keep API generic**: rely on userland wrappers. Rejected because repeated
  boilerplate leads to divergent practices and error-prone framing.
- **Adopt gRPC semantics**: implement gRPC-compatible streaming. Rejected due to
  scope creep; we aim for lighter-weight integration tailored to QUIC.

## 6. Risks and Mitigations

- **API Surface Growth**: Additional helper methods increase maintenance burden.
  Mitigate by versioning the experimental APIs under a feature flag (`ml-serving`).
- **Performance Regressions**: Improper batching may increase latency. Provide
  benchmarks comparing baseline vs helper flows; fall back to raw APIs if needed.
- **Interoperability**: Custom streaming profiles may drift from third-party
  expectations. Document wire formats clearly and keep generic APIs available.
- **Connection Handoff Complexity**: Broker-mediated swapping introduces
  security and state-synchronisation risks. Implement staged handoff with
  timeouts, conservative retry limits, and exhaustive integration tests.

## 7. Conclusion

This RFC proposes ergonomic enhancements for both batched and streaming inference
scenarios, aligning RpcNet with modern ML serving requirements. The plan focuses
on developer-friendly helpers, targeted streaming profiles, resource coordination
patterns, and supporting documentation.
