# Python Extension Build Guide

This guide explains how to properly build and work with the RpcNet Python extension module.

## TL;DR - Quick Start

```bash
# Build the Python extension (clean build)
make python-build

# Run Python tests
make python-test

# Clean Python artifacts
make python-clean
```

## Code Generation with Automatic Build

The `rpcnet-gen` CLI tool can generate Python bindings and automatically build the extension in one step:

```bash
# Generate Python bindings and build automatically
rpcnet-gen --input service.rpc.rs --output src/generated --python

# Generate without building (just code generation)
rpcnet-gen --input service.rpc.rs --output src/generated --python --no-build
```

### How It Works

When you use the `--python` flag, `rpcnet-gen` will:

1. ✅ Generate Python client, server, and types code
2. ✅ Automatically run `maturin develop --features extension-module`
3. ✅ Verify the module imports correctly
4. ✅ Display helpful usage examples

### When to Use Each Approach

**Use `rpcnet-gen --python`** when:
- Generating new service bindings from `.rpc.rs` files
- You want code generation + build in one command
- Starting fresh with a new service

**Use `make python-build`** when:
- Working on the core Rust implementation (`src/python/`)
- No service definition changes, just Rust code changes
- Need a guaranteed clean build
- Troubleshooting import/build issues

### Error Handling

If maturin is not found or the build fails:

```
⚠️  Warning: maturin not found in PATH
   Install with: pip install maturin
   Or skip build with: --no-build flag
```

The tool will still generate the Python code, and you can build manually:

```bash
# Install maturin if needed
pip install maturin

# Build manually
maturin develop --features extension-module

# Or use the Makefile
make python-build
```

### Example Workflow

```bash
# 1. Create your service definition
cat > greeting.rpc.rs <<EOF
#[rpcnet::service]
pub trait Greeting {
    async fn hello(&self, req: HelloRequest) -> Result<HelloResponse, Error>;
}

pub struct HelloRequest { pub name: String }
pub struct HelloResponse { pub message: String }
pub enum Error { InvalidName }
EOF

# 2. Generate + build Python bindings
rpcnet-gen --input greeting.rpc.rs --output src/generated --python

# 3. Use in Python
python3 <<EOF
import greeting
client = await greeting.GreetingClient.connect("127.0.0.1:8080", "cert.pem")
response = await client.hello({"name": "Alice"})
print(response)
EOF
```

## The Problem

The Python extension module (`_rpcnet`) can have stale artifacts that cause issues:

1. **Old .so files** lingering in various directories
2. **Python import caching** picking up old versions
3. **Inconsistent builds** when switching between features
4. **Missing classes** due to partial/stale builds

### Symptoms

- `AttributeError: module '_rpcnet' has no attribute 'ClusterConfig'`
- Missing cluster classes even though they're in the source code
- Tests collecting 0 items / being skipped
- Import errors for `_rpcnet` module

## The Solution

We've implemented a robust build system that ensures clean builds:

### 1. Automated Build Script

**`./scripts/build_python.sh`** - Does the following automatically:

1. ✅ Removes all .so files (`_rpcnet*.so`, `librpcnet*.so`, etc.)
2. ✅ Cleans Python cache (`__pycache__`, `*.pyc`, etc.)
3. ✅ Removes build artifacts (`target/wheels/`, `.pytest_cache/`, etc.)
4. ✅ Cleans cargo build for the rpcnet package
5. ✅ Activates the venv automatically
6. ✅ Builds with correct features (`--features extension-module`)
7. ✅ Verifies the module imports correctly
8. ✅ Lists all available classes for confirmation

### 2. Makefile Targets

Convenient commands for development:

```bash
# Setup Python environment (one-time)
make python-setup

# Build extension module (use this for development)
make python-build

# Build in release mode (faster, for benchmarks)
make python-build-release

# Run Python integration tests
make python-test

# Clean all Python artifacts
make python-clean
```

### 3. Updated .gitignore

Comprehensive Python artifact patterns to prevent committing build files:

- `.so`, `.dylib`, `.pyd` files
- `__pycache__/`, `*.pyc`, `*.pyo`
- Virtual environments (`.venv/`, `venv/`)
- Build directories (`build/`, `dist/`, `*.egg-info/`)
- Test artifacts (`.pytest_cache/`, `.coverage`)

## Development Workflow

### Initial Setup

```bash
# 1. Create and setup Python environment
make python-setup

# 2. Build the extension
make python-build
```

### Daily Development

```bash
# After changing Python bindings in src/python/
make python-build

# Run tests
make python-test
```

### Troubleshooting

If you encounter import issues:

```bash
# 1. Clean everything
make python-clean

# 2. Clean cargo build
cargo clean -p rpcnet

# 3. Rebuild
make python-build
```

### Verifying the Build

After building, you should see output like:

```
📦 Available _rpcnet classes:
   - AsyncStream
   - Cluster ✅
   - ClusterConfig ✅
   - ClusterEventReceiver ✅
   - GossipConfig ✅
   - HealthCheckConfig ✅
   - PoolConfig ✅
   - QuicClient ✅
   - RpcClient
   - RpcConfig
   - RpcServer
   ...
```

All cluster classes marked with ✅ should be present.

## Manual Build (Advanced)

If you need to build manually:

```bash
# Activate venv
source .venv/bin/activate

# Build with maturin
maturin develop --features extension-module

# Verify
python3 -c "import _rpcnet; print(dir(_rpcnet))"
```

## CI/CD Integration

For continuous integration:

```yaml
- name: Setup Python Environment
  run: make python-setup

- name: Build Python Extension
  run: make python-build

- name: Run Python Tests
  run: make python-test
```

## Common Pitfalls

### ❌ Using system Python instead of venv

```bash
# BAD - uses system python
python3 -c "import _rpcnet"

# GOOD - uses venv python
.venv/bin/python3 -c "import _rpcnet"

# BEST - use the build script
make python-build
```

### ❌ Not cleaning before rebuild

```bash
# BAD - might pick up stale artifacts
maturin develop --features extension-module

# GOOD - clean first
make python-clean && make python-build
```

### ❌ Forgetting features flag

```bash
# BAD - builds without extension-module feature
maturin develop

# GOOD - includes correct features
maturin develop --features extension-module
```

## Architecture Notes

- **Extension module name**: `_rpcnet` (with underscore prefix)
- **Feature flag**: `extension-module` (enables PyO3 extension mode)
- **Build tool**: `maturin` (Rust-Python bridge)
- **Installation**: Editable mode (links to source, not copies)

## Why This System Works

1. **Idempotent**: Running `make python-build` multiple times is safe
2. **Comprehensive**: Cleans all known artifact locations
3. **Verified**: Automatically tests that the module imports correctly
4. **Informative**: Shows exactly what classes are available
5. **Documented**: Clear error messages guide the user

## Further Reading

- [Python Extension README](../tests/README.md) - Test documentation
- [Maturin Guide](https://www.maturin.rs/) - Build tool documentation
- [PyO3 Guide](https://pyo3.rs/) - Rust-Python bindings
