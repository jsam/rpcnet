# Python-to-Rust MessagePack Serialization Fix

## Summary

Fixed Python-to-Rust RPC communication in the rpcnet cluster example. The issue was a MessagePack serialization format mismatch between Python and Rust.

## Problem

When Python clients tried to call Rust RPC services (like the director's `get_worker` method), the requests would time out. The director logs showed:

```
⚠️  Not a regular RPC request (tried 101 bytes): Syntax("invalid type: integer `1`, expected struct RpcRequest")
First 20 bytes: [1, 0, 0, 0, 0, 0, 0, 0, 27, 0, 0, 0, 0, 0, 0, 0, 68, 105, 114, 101]
```

The Python client was serializing structs as MessagePack **arrays** (compact format), but the Rust server expected MessagePack **maps** with named fields.

## Root Cause

In `src/python/serde.rs`, the `python_to_msgpack_py` function was using `rmp_serde::to_vec(&val)` which serializes the `rmpv::Value` enum wrapper instead of writing the raw MessagePack structure.

## Solution

### 1. Fixed Python MessagePack Serialization

**File:** `src/python/serde.rs` (Line 182-188)

**Before:**
```rust
let bytes = rmp_serde::to_vec(&val).map_err(|e| {
    pyo3::exceptions::PyValueError::new_err(format!(
        "MessagePack serialization failed: {}",
        e
    ))
})?;
```

**After:**
```rust
let mut bytes = Vec::new();
rmpv::encode::write_value(&mut bytes, &val).map_err(|e| {
    pyo3::exceptions::PyValueError::new_err(format!(
        "MessagePack serialization failed: {}",
        e
    ))
})?;
```

**Why:** `rmpv::encode::write_value` writes the actual MessagePack bytes directly, preserving the map structure (named fields) that Rust's serde deserializer expects.

### 2. Added Result Unwrapping for Streaming RPCs

**File:** `src/codegen/python_generator.rs` (Lines 599-614)

Streaming RPC methods return `Result<T, E>`, which gets serialized as `{"Ok": {...}}` or `{"Err": {...}}`. Added code to the generated Python client to unwrap this:

```python
# Unwrap Result if present (Rust streaming methods return Result<T, E>)
if isinstance(response_dict, dict) and 'Ok' in response_dict:
    response_dict = response_dict['Ok']
elif isinstance(response_dict, dict) and 'Err' in response_dict:
    # Handle error variant - could raise exception or yield error
    error_dict = response_dict['Err']
    raise Exception(f"RPC error: {error_dict}")
```

### 3. Added Missing `infer` Method

**File:** `examples/cluster/inference.rpc.rs` (Line 27)

Added a non-streaming `infer` method to complement the existing `generate` streaming method:

```rust
async fn infer(&self, request: InferenceRequest) -> Result<InferenceResponse, InferenceError>;
```

**File:** `examples/cluster/src/worker.rs` (Lines 28-51)

Implemented the method in the worker handler.

## Verification

After the fix, all three Python example clients work correctly:

1. ✅ `python_client.py` - Simple RPC calls with load balancing
2. ✅ `python_streaming_client.py` - Non-streaming `infer` calls
3. ✅ `python_real_streaming_client.py` - Bidirectional streaming `generate` calls

### Test Output

```bash
$ .venv/bin/python examples/python/cluster/python_real_streaming_client.py

✅ Connected to director at 127.0.0.1:61000
✅ Got worker assignment: worker-a at 127.0.0.1:62001
✅ Connected to worker at 127.0.0.1:62001

📥 Response 1: InferenceResponseConnected
   🔗 Connected to worker: worker-a

📥 Response 2-6: InferenceResponseToken
   ✅ Token responses processed successfully

📊 Total responses received: 6
✅ Bidirectional Streaming Demo Completed Successfully!
```

## Technical Details

### MessagePack Format Comparison

**Array Format (old, broken):**
```
[131, 1, "DirectorRegistry.get_worker", [...]]
     ↑ RpcRequest serialized as 3-element array
```

**Map Format (new, working):**
```
{131,
  "id": 1,
  "method": "DirectorRegistry.get_worker",
  "params": [...]
}
  ↑ RpcRequest serialized as map with field names
```

### Why the Fix Works

1. **Python side:** `rmpv::encode::write_value` writes raw MessagePack bytes that represent a map with named keys
2. **Rust side:** `rmp_serde::from_slice::<RpcRequest>` can deserialize from map format
3. **Result:** Python's dict `{"connection_id": None, "prompt": "..."}` → MessagePack map `{0x82, 0xa6, "prompt", ...}` → Rust's `GetWorkerRequest` struct

## Files Modified

1. `src/python/serde.rs` - Fixed MessagePack serialization
2. `src/codegen/python_generator.rs` - Added Result unwrapping for streaming
3. `examples/cluster/inference.rpc.rs` - Added `infer` method
4. `examples/cluster/src/worker.rs` - Implemented `infer` method
5. `src/python/error.rs` - Fixed test to use String instead of bincode error
6. `src/codegen/generator.rs` - Collapsed nested if-lets (clippy fix)

## Build & Test

```bash
# Rebuild Python bindings
maturin develop --features python,pyo3/extension-module --release

# Rebuild cluster examples
cargo build --manifest-path examples/cluster/Cargo.toml --release

# Regenerate Python code
cargo run --bin rpcnet-gen --features codegen,python -- \
  --input examples/cluster/director_registry.rpc.rs \
  --output examples/python/cluster/generated --python

cargo run --bin rpcnet-gen --features codegen,python -- \
  --input examples/cluster/inference.rpc.rs \
  --output examples/python/cluster/generated --python

# Run tests
cargo test --features python --lib python
```

## Related Issues

- MessagePack serialization compatibility between Python and Rust
- Streaming RPC Result type handling
- Code generation for Python clients

## Date

November 7, 2025
