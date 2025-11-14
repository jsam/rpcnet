#!/bin/bash
#
# Test All Streaming Examples
# This script starts all 3 servers and runs test clients against them
#

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR/../../.."

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Testing All Streaming Examples${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Kill any existing streaming processes
echo -e "${YELLOW}Cleaning up old processes...${NC}"
pkill -f "streaming.*\.py" 2>/dev/null || true
sleep 1

# Start all 3 servers in background
echo -e "${GREEN}Starting servers...${NC}"
.venv/bin/python examples/python/streaming/server_streaming_example.py > /tmp/server_stream_test.log 2>&1 &
SERVER_PID=$!
echo "  ✓ Server Streaming on port 9001 (PID: $SERVER_PID)"

.venv/bin/python examples/python/streaming/client_streaming_example.py > /tmp/client_stream_test.log 2>&1 &
CLIENT_PID=$!
echo "  ✓ Client Streaming on port 9002 (PID: $CLIENT_PID)"

.venv/bin/python examples/python/streaming/bidirectional_streaming_example.py > /tmp/bidir_stream_test.log 2>&1 &
BIDIR_PID=$!
echo "  ✓ Bidirectional Streaming on port 9003 (PID: $BIDIR_PID)"

echo ""
echo -e "${YELLOW}Waiting for servers to start...${NC}"
sleep 3

# Run test clients
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Running Test Clients${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Test 1: Server Streaming
echo -e "${GREEN}1. Testing Server Streaming (1→N)${NC}"
echo -e "${YELLOW}   Port: 9001${NC}"
if .venv/bin/python examples/python/streaming/test_server_streaming.py 2>&1 | tail -5; then
    echo -e "${GREEN}   ✓ Server Streaming PASSED${NC}"
else
    echo -e "${RED}   ✗ Server Streaming FAILED${NC}"
fi
echo ""

# Test 2: Client Streaming
echo -e "${GREEN}2. Testing Client Streaming (N→1)${NC}"
echo -e "${YELLOW}   Port: 9002${NC}"
if .venv/bin/python examples/python/streaming/test_client_streaming.py 2>&1 | tail -5; then
    echo -e "${GREEN}   ✓ Client Streaming PASSED${NC}"
else
    echo -e "${RED}   ✗ Client Streaming FAILED${NC}"
fi
echo ""

# Test 3: Bidirectional Streaming
echo -e "${GREEN}3. Testing Bidirectional Streaming (N→M)${NC}"
echo -e "${YELLOW}   Port: 9003${NC}"
if .venv/bin/python examples/python/streaming/test_bidirectional_streaming.py 2>&1 | tail -5; then
    echo -e "${GREEN}   ✓ Bidirectional Streaming PASSED${NC}"
else
    echo -e "${RED}   ✗ Bidirectional Streaming FAILED${NC}"
fi
echo ""

# Cleanup
echo -e "${YELLOW}Cleaning up...${NC}"
kill $SERVER_PID $CLIENT_PID $BIDIR_PID 2>/dev/null || true
sleep 1
pkill -f "streaming.*\.py" 2>/dev/null || true

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}All Tests Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Server logs available at:"
echo "  /tmp/server_stream_test.log"
echo "  /tmp/client_stream_test.log"
echo "  /tmp/bidir_stream_test.log"
