#!/bin/bash

# Test runner script for Python bindings
# This script ensures the module is built and runs the test suite

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}===================================${NC}"
echo -e "${YELLOW}RpcNet Python Bindings Test Runner${NC}"
echo -e "${YELLOW}===================================${NC}"
echo ""

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Must be run from the rpcnet root directory${NC}"
    exit 1
fi

# Check if pytest is installed
if ! command -v pytest &> /dev/null; then
    echo -e "${RED}Error: pytest is not installed${NC}"
    echo "Install with: pip install pytest pytest-asyncio"
    exit 1
fi

# Check if certificates exist
if [ ! -f "certs/test_cert.pem" ] || [ ! -f "certs/test_key.pem" ]; then
    echo -e "${YELLOW}Generating test certificates...${NC}"
    mkdir -p certs
    cd certs
    openssl req -x509 -newkey rsa:4096 -keyout test_key.pem -out test_cert.pem \
        -days 365 -nodes -subj "/CN=localhost" 2>/dev/null
    cd ..
    echo -e "${GREEN}✓ Certificates generated${NC}"
fi

# Build the Python module
echo -e "${YELLOW}Building Python module...${NC}"
if command -v maturin &> /dev/null; then
    # Use maturin if available
    maturin develop --features python --quiet
    echo -e "${GREEN}✓ Module built with maturin${NC}"
else
    # Fall back to cargo build
    cargo build --release --features python
    echo -e "${GREEN}✓ Module built with cargo${NC}"
    echo -e "${YELLOW}Note: Install maturin for better Python integration: pip install maturin${NC}"
fi

echo ""
echo -e "${YELLOW}Running tests...${NC}"
echo ""

# Run pytest with options
pytest python_tests/ \
    -v \
    --tb=short \
    --asyncio-mode=auto \
    "$@"

# Check exit code
if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}===================================${NC}"
    echo -e "${GREEN}✓ All tests passed!${NC}"
    echo -e "${GREEN}===================================${NC}"
else
    echo ""
    echo -e "${RED}===================================${NC}"
    echo -e "${RED}✗ Some tests failed${NC}"
    echo -e "${RED}===================================${NC}"
    exit 1
fi
