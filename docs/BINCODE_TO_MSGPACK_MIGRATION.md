# Bincode to MessagePack Migration

## Overview

RpcNet has migrated from bincode to MessagePack for all serialization. This change provides better cross-language support while maintaining excellent performance.

## What Changed

### Before (Bincode)
```rust
// Old code
let payload = bincode::serialize(&data)?;
let response = client.call("method", payload).await?;
let result: MyType = bincode::deserialize(&response)?;
```

### After (MessagePack)
```rust
// New code
let payload = rmp_serde::to_vec(&data)?;
let response = client.call("method", payload).await?;
let result: MyType = rmp_serde::from_slice(&response)?;
```

## Why MessagePack?

1. **Cross-Language Support**: Native Python, JavaScript, and other language bindings
2. **Performance**: Comparable to bincode in most scenarios
3. **Debugging**: Self-describing format makes debugging easier
4. **Consistency**: One serialization format for all clients

## Migration Guide

### Rust Code

Replace all `bincode` calls with `rmp_serde`:

```rust
// Serialization
bincode::serialize(&value)?        → rmp_serde::to_vec(&value)?

// Deserialization  
bincode::deserialize(&bytes)?      → rmp_serde::from_slice(&bytes)?

// Error handling
.map_err(RpcError::SerializationError)?   
  → .map_err(|e| RpcError::SerializationError(e.to_string()))?
```

### Dependencies

Update `Cargo.toml`:
```toml
[dependencies]
# Remove: bincode = "1.3"
rmp-serde = "1.3"    # Add if not already present
rmpv = "1.3"         # For Python interop
```

### Python Code

No changes needed! Python bindings automatically use MessagePack via `_rpcnet.python_to_msgpack_py()` and `_rpcnet.msgpack_to_python_py()`.

## Performance Impact

MessagePack performance is comparable to bincode:
- Serialization: ~5% slower
- Deserialization: ~3% slower  
- Size: Within 5% of bincode for most payloads

The cross-language compatibility benefits far outweigh the minimal performance difference.

## Compatibility

- ✅ Rust ↔ Rust: Fully compatible
- ✅ Python ↔ Rust: Fully compatible
- ✅ Mixed language clusters: Fully supported

## Rollout

This change is **breaking** - you must update both clients and servers simultaneously. Mixing bincode and MessagePack will cause deserialization errors.

## Date

November 10, 2025
