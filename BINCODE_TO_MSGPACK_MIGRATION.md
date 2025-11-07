# Bincode to MessagePack Migration Guide

## Overview
This document tracks the migration from `bincode` to `rmp_serde` (MessagePack) serialization throughout the RpcNet codebase.

## Status: COMPLETED ✅

### Completed ✅
1. ✅ Updated `RpcError::SerializationError` to use `String` instead of `bincode::Error`
2. ✅ Added `From<rmp_serde::encode::Error>` and `From<rmp_serde::decode::Error>` implementations for `RpcError`
3. ✅ Replaced all bincode calls in src/lib.rs (~20 occurrences)
4. ✅ Updated SWIM gossip protocol (message.rs, swim.rs, incarnation.rs)
5. ✅ Updated code generator templates (generator.rs) to emit rmp_serde
6. ✅ Updated all test files (~104 occurrences across 14 files)
7. ✅ Updated simple example files (basic_client.rs, basic_server.rs, etc.)
8. ✅ Removed bincode dependency from Cargo.toml
9. ✅ Regenerated all example code (7 examples with updated generator)
10. ✅ Fixed buffer size optimization issue (removed 16-byte minimum check)
11. ✅ **ALL TESTS PASS: 183/183 tests passing** ✨
12. ✅ Added `StreamError<RpcError>` conversions for rmp_serde errors (src/streaming.rs)
13. ✅ Fixed code generator to handle `Stream<Item = Result<T, E>>` pattern (src/codegen/generator.rs)
14. ✅ Cluster example builds successfully with complex streaming pattern

### Remaining Work 📋
- [ ] Update main documentation to reflect MessagePack migration
- [ ] Consider major version bump (v0.x.x → v1.0.0)
- [ ] Update CHANGELOG.md with breaking changes notice
- [x] **FIXED:** Code generator now properly handles streaming methods that return `Result<T, E>` items (cluster example builds successfully)

## Search and Replace Patterns

### Core Replacements

#### Serialization
```rust
// OLD:
bincode::serialize(&value).map_err(RpcError::SerializationError)

// NEW:
rmp_serde::to_vec(&value).map_err(|e| RpcError::SerializationError(format!("MessagePack encode error: {}", e)))

// OR (with From impl):
rmp_serde::to_vec(&value)?
```

#### Deserialization
```rust
// OLD:
bincode::deserialize(&bytes).map_err(RpcError::SerializationError)?

// NEW:
rmp_serde::from_slice(&bytes).map_err(|e| RpcError::SerializationError(format!("MessagePack decode error: {}", e)))?

// OR (with From impl):
rmp_serde::from_slice(&bytes)?
```

### Error Handling
```rust
// OLD:
SerializationError(#[from] bincode::Error),

// NEW:
SerializationError(String),

// With From implementations for:
// - rmp_serde::encode::Error
// - rmp_serde::decode::Error
```

## Files to Modify

### Core Library (src/)
- [x] `src/lib.rs` - RpcError definition (DONE)
- [ ] `src/lib.rs` - All bincode calls (~20 occurrences)
- [ ] `src/cluster/gossip/message.rs`
- [ ] `src/cluster/gossip/swim.rs`
- [ ] `src/cluster/incarnation.rs`
- [ ] `src/python/serde.rs` - Rename misleading function names
- [ ] `src/python/error.rs`

### Code Generator (src/codegen/)
- [ ] `src/codegen/generator.rs` - Update Rust templates
- [ ] `src/codegen/python_generator.rs` - Already uses MessagePack

### Examples
- [ ] Regenerate all with updated codegen
- [ ] Update hand-written serialization calls

### Tests
- [ ] `tests/*.rs` - Update all test files (~30-50 files)

## Breaking Changes

### Wire Protocol
⚠️ **MAJOR BREAKING CHANGE**: Binary protocol is incompatible between bincode and MessagePack.

- Old clients cannot communicate with new servers
- Old servers cannot communicate with new clients
- Cluster nodes must all upgrade simultaneously

### API Changes
- `RpcError::SerializationError` now contains `String` instead of `bincode::Error`
- Error messages will have different format

## Migration Steps for Users

### For Application Code
```rust
// OLD
use rpcnet::RpcClient;
let params = bincode::serialize(&my_data)?;
let response = client.call("method", params).await?;
let result: MyType = bincode::deserialize(&response)?;

// NEW
use rpcnet::RpcClient;
let params = rmp_serde::to_vec(&my_data)?;
let response = client.call("method", params).await?;
let result: MyType = rmp_serde::from_slice(&response)?;
```

### For Generated Code
Simply regenerate using the updated `rpcnet-gen` tool:
```bash
rpcnet-gen --input service.rpc.rs --output src/generated
```

## Testing Strategy

1. **Unit Tests**: Update all unit tests to use MessagePack
2. **Integration Tests**: Verify end-to-end communication works
3. **SWIM Protocol Tests**: Ensure cluster communication works
4. **Python Interop Tests**: Already using MessagePack, should continue working
5. **Performance Tests**: Benchmark to compare with bincode

## Rollback Plan

If issues arise:
1. Revert to previous git commit
2. This is a clean big-bang migration, so rollback is straightforward
3. No partial migration states to worry about

## Performance Considerations

MessagePack vs Bincode:
- **Size**: MessagePack typically 5-10% larger
- **Speed**: Bincode is ~10-30% faster for Rust types
- **Compatibility**: MessagePack is language-agnostic, better for polyglot systems
- **Compactness**: MessagePack can encode very small messages in <16 bytes (unlike bincode which has larger minimum overhead)

### Important Fixes Applied

**1. Buffer Size Optimization Removed**: The original code had a 16-byte minimum buffer size check before attempting deserialization. This was based on bincode's encoding characteristics. MessagePack is more compact and can encode small responses (e.g., `RpcResponse` with a 4-byte payload) in less than 16 bytes. The minimum size check was removed to ensure proper deserialization of all MessagePack messages.

**2. Complex Streaming Pattern Fix**: The code generator now properly handles streaming methods that return `Stream<Item = Result<T, E>>`:
- **Problem**: When stream items are themselves `Result<T, E>`, deserializing creates nested Results that don't match the expected signature
- **Solution**: Added detection logic (`is_result_type()` and `extract_result_inner_types()`) to identify Result stream items
- **Implementation**: For Result items, deserialize directly to `Result<T, E>` and return it without nesting
- **Error Handling**: Transport/timeout errors panic with a clear message since they can't be converted to user-defined error types
- **Files Modified**:
  - `src/streaming.rs`: Added `From` implementations for `StreamError<RpcError>` from rmp_serde errors
  - `src/codegen/generator.rs`: Added Result detection and special deserialization code generation
- **Verification**: Cluster example builds successfully with the complex streaming pattern

## Version Bump

This migration requires:
- **Major version bump**: e.g., v0.x.x → v1.0.0 (or v1.x.x → v2.0.0)
- Update `Cargo.toml` version
- Update CHANGELOG.md with breaking changes notice
