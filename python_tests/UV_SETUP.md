# Using UV with RpcNet Python Bindings

This guide shows how to use [uv](https://github.com/astral-sh/uv) (a fast Python package manager) with a local virtual environment for developing and testing the Python bindings.

## Why UV?

- **Fast**: 10-100x faster than pip
- **Reliable**: Better dependency resolution
- **Modern**: Built in Rust with great UX
- **Compatible**: Drop-in replacement for pip/pip-tools/virtualenv

## Installation

### Install UV

```bash
# macOS/Linux
curl -LsSf https://astral.sh/uv/install.sh | sh

# Or with Homebrew (macOS)
brew install uv

# Or with pip
pip install uv
```

## Quick Start

### 1. Create Virtual Environment

```bash
# From the rpcnet root directory
uv venv

# This creates a .venv directory
```

### 2. Activate the Environment

```bash
# macOS/Linux
source .venv/bin/activate

# Or with uv (automatically activates)
# uv will auto-detect and use .venv for subsequent commands
```

### 3. Install Dependencies

```bash
# Install test dependencies
uv pip install -r python_tests/requirements.txt

# Or install specific packages
uv pip install pytest pytest-asyncio maturin
```

### 4. Build the Module

```bash
# Using maturin (installed via uv)
uv run maturin develop --features python

# Or activate venv first, then run
source .venv/bin/activate
maturin develop --features python
```

### 5. Run Tests

```bash
# With uv run (automatically uses .venv)
uv run pytest python_tests/ -v

# Or with activated venv
source .venv/bin/activate
pytest python_tests/ -v

# Or use the test runner
uv run python python_tests/run_tests.py
```

## Complete Workflow

```bash
# 1. Create and setup environment
cd /Users/alessandroaresta/rpcnet
uv venv
uv pip install -r python_tests/requirements.txt

# 2. Build the module
uv run maturin develop --features python

# 3. Run tests
uv run pytest python_tests/ -v

# 4. Development: rebuild after Rust changes
uv run maturin develop --features python

# 5. Run specific tests
uv run pytest python_tests/test_serialization.py -v
```

## UV Commands Reference

### Environment Management

```bash
# Create virtual environment
uv venv                          # Creates .venv
uv venv myenv                    # Creates myenv/
uv venv --python 3.11            # Use specific Python version

# Remove environment
rm -rf .venv
```

### Package Installation

```bash
# Install packages
uv pip install pytest            # Single package
uv pip install -r requirements.txt  # From file
uv pip install -e .              # Editable install

# Install with extras
uv pip install "rpcnet[dev]"

# Upgrade packages
uv pip install --upgrade pytest

# Uninstall
uv pip uninstall pytest
```

### Running Commands

```bash
# Run command in venv (auto-activates)
uv run python script.py
uv run pytest
uv run maturin develop

# Run with specific venv
uv run --venv .venv pytest
```

### Dependency Management

```bash
# Generate requirements.txt from installed packages
uv pip freeze > requirements.txt

# List installed packages
uv pip list

# Show package info
uv pip show pytest
```

## Project Structure with UV

```
rpcnet/
├── .venv/                    # UV virtual environment
├── python_tests/
│   ├── requirements.txt      # Test dependencies
│   ├── conftest.py
│   ├── test_*.py
│   └── run_tests.py
├── src/
│   └── python/               # Rust Python bindings
├── Cargo.toml
└── pyproject.toml            # Optional: for UV project config
```

## Optional: pyproject.toml

For better UV integration, you can create a `pyproject.toml`:

```toml
[project]
name = "rpcnet"
version = "0.1.0"
description = "Low-latency RPC library with Python bindings"
requires-python = ">=3.8"
dependencies = []

[project.optional-dependencies]
dev = [
    "pytest>=7.0.0",
    "pytest-asyncio>=0.21.0",
    "maturin>=1.0.0",
]
test = [
    "pytest>=7.0.0",
    "pytest-asyncio>=0.21.0",
    "pytest-cov>=4.0.0",
]

[build-system]
requires = ["maturin>=1.0,<2.0"]
build-backend = "maturin"

[tool.maturin]
features = ["python"]
module-name = "_rpcnet"
```

Then use:

```bash
# Install with dev dependencies
uv pip install -e ".[dev]"

# Install with test dependencies
uv pip install -e ".[test]"
```

## UV Test Runner Integration

Update the test runners to use UV:

### Modified run_tests.sh

```bash
#!/bin/bash

# Check if uv is available
if command -v uv &> /dev/null; then
    echo "Using UV..."

    # Ensure venv exists
    if [ ! -d ".venv" ]; then
        uv venv
    fi

    # Install dependencies
    uv pip install -r python_tests/requirements.txt

    # Build module
    uv run maturin develop --features python

    # Run tests
    uv run pytest python_tests/ -v
else
    echo "UV not found, falling back to pip..."
    # ... existing pip-based logic
fi
```

### Modified run_tests.py

```python
import subprocess
import shutil

def has_uv():
    """Check if uv is available."""
    return shutil.which("uv") is not None

def run_with_uv():
    """Run tests using UV."""
    print("Using UV for faster package management...")

    # Ensure venv exists
    if not Path(".venv").exists():
        subprocess.run(["uv", "venv"], check=True)

    # Install dependencies
    subprocess.run([
        "uv", "pip", "install",
        "-r", "python_tests/requirements.txt"
    ], check=True)

    # Build module
    subprocess.run([
        "uv", "run", "maturin", "develop",
        "--features", "python"
    ], check=True)

    # Run tests
    subprocess.run([
        "uv", "run", "pytest",
        "python_tests/", "-v"
    ], check=True)
```

## Development Workflow Tips

### Fast Iteration

```bash
# Terminal 1: Watch and rebuild on Rust changes
uv run cargo watch -x "build --features python"

# Terminal 2: Run tests
uv run pytest python_tests/ -v --watch
```

### Quick Rebuild and Test

```bash
# Rebuild and test in one command
uv run maturin develop --features python && uv run pytest python_tests/ -v
```

### Shell Alias (Optional)

Add to your `.bashrc` or `.zshrc`:

```bash
alias rpctest='uv run maturin develop --features python && uv run pytest python_tests/ -v'
alias rpcbuild='uv run maturin develop --features python'
```

Then just run:
```bash
rpctest    # Build and test
rpcbuild   # Just build
```

## Performance Comparison

```bash
# Traditional pip
time pip install -r python_tests/requirements.txt
# ~15-30 seconds

# With UV
time uv pip install -r python_tests/requirements.txt
# ~1-3 seconds ⚡
```

## Troubleshooting

### UV Not Finding Python

```bash
# Specify Python explicitly
uv venv --python python3.11
uv venv --python /usr/local/bin/python3.11
```

### Module Not Found After Build

```bash
# Make sure you built in the correct venv
uv run maturin develop --features python

# Or check the venv is active
which python
# Should show: /path/to/rpcnet/.venv/bin/python
```

### UV Cache Issues

```bash
# Clear UV cache if needed
uv cache clean
```

### Permissions Issues

```bash
# UV installs to user directory by default
# No sudo needed!
```

## CI/CD Integration

### GitHub Actions

```yaml
- name: Setup UV
  uses: astral-sh/setup-uv@v1

- name: Create venv and install deps
  run: |
    uv venv
    uv pip install -r python_tests/requirements.txt

- name: Build and test
  run: |
    uv run maturin develop --features python
    uv run pytest python_tests/ -v
```

### GitLab CI

```yaml
test:python:
  before_script:
    - curl -LsSf https://astral.sh/uv/install.sh | sh
    - uv venv
    - uv pip install -r python_tests/requirements.txt
  script:
    - uv run maturin develop --features python
    - uv run pytest python_tests/ -v
```

## Summary

Using UV with RpcNet Python bindings:

```bash
# One-time setup
uv venv
uv pip install -r python_tests/requirements.txt

# Daily development
uv run maturin develop --features python  # Rebuild
uv run pytest python_tests/ -v            # Test

# Or combined
uv run maturin develop --features python && uv run pytest python_tests/ -v
```

**Benefits:**
- ⚡ 10-100x faster than pip
- 🔒 Better dependency resolution
- 🎯 Automatic venv detection
- 🚀 Great developer experience

For more information, see the [UV documentation](https://github.com/astral-sh/uv).
