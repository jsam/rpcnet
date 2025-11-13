#!/bin/bash
#
# Python Streaming Examples Runner
#
# This script helps you run the Python streaming RPC examples.
# Each example demonstrates a different streaming pattern.

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR/../../.."

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Python Streaming RPC Examples${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Check if virtual environment exists
if [ ! -d ".venv" ]; then
    echo -e "${RED}Error: Virtual environment not found!${NC}"
    echo "Please run: python3 -m venv .venv && .venv/bin/pip install maturin"
    exit 1
fi

# Check if Python bindings are built with streaming support
echo -e "${YELLOW}Checking Python bindings...${NC}"
if ! .venv/bin/python -c "import _rpcnet; s = _rpcnet.RpcServer(_rpcnet.RpcConfig('certs/test_cert.pem', 'certs/test_key.pem', '127.0.0.1:0', 'localhost', 5)); assert hasattr(s, 'register_server_streaming')" 2>/dev/null; then
    echo -e "${YELLOW}Python bindings missing or outdated. Rebuilding with streaming support...${NC}"
    maturin develop --features python
    echo ""
    echo -e "${GREEN}✓ Python bindings rebuilt${NC}"
    echo ""
fi

# Check if certificates exist
if [ ! -f "certs/test_cert.pem" ] || [ ! -f "certs/test_key.pem" ]; then
    echo -e "${YELLOW}TLS certificates not found. Creating self-signed certificates...${NC}"
    mkdir -p certs
    openssl req -x509 -newkey rsa:4096 -keyout certs/test_key.pem -out certs/test_cert.pem -days 365 -nodes -subj "/CN=localhost" 2>/dev/null
    echo -e "${GREEN}✓ Certificates created${NC}"
    echo ""
fi

# Function to display menu
show_menu() {
    echo -e "${BLUE}Available Examples:${NC}"
    echo ""
    echo "  1) Server Streaming (1→N)"
    echo "     Port: 9001"
    echo "     Pattern: One request → Multiple responses"
    echo "     Use case: Streaming data feeds, progress updates"
    echo ""
    echo "  2) Client Streaming (N→1)"
    echo "     Port: 9002"
    echo "     Pattern: Multiple requests → One response"
    echo "     Use case: File uploads, data aggregation"
    echo ""
    echo "  3) Bidirectional Streaming (N→M)"
    echo "     Port: 9003"
    echo "     Pattern: Multiple requests → Multiple responses"
    echo "     Use case: Real-time chat, interactive processing"
    echo ""
    echo "  4) Run all examples (in separate terminals)"
    echo ""
    echo "  0) Exit"
    echo ""
}

# Function to run an example
run_example() {
    local example=$1
    local name=$2
    local port=$3

    echo -e "${GREEN}Starting ${name}...${NC}"
    echo -e "${YELLOW}Server will listen on 127.0.0.1:${port}${NC}"
    echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
    echo ""

    .venv/bin/python -u "$SCRIPT_DIR/$example"
}

# Function to run all examples
run_all() {
    echo -e "${YELLOW}This will open 3 terminal windows with each example.${NC}"
    echo -e "${YELLOW}Make sure you're running this from a terminal that supports 'osascript' (macOS)${NC}"
    echo ""
    read -p "Continue? (y/n) " -n 1 -r
    echo ""

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        # Detect OS
        if [[ "$OSTYPE" == "darwin"* ]]; then
            # macOS
            osascript -e "tell application \"Terminal\" to do script \"cd $(pwd) && .venv/bin/python -u examples/python/streaming/server_streaming_example.py\""
            sleep 0.5
            osascript -e "tell application \"Terminal\" to do script \"cd $(pwd) && .venv/bin/python -u examples/python/streaming/client_streaming_example.py\""
            sleep 0.5
            osascript -e "tell application \"Terminal\" to do script \"cd $(pwd) && .venv/bin/python -u examples/python/streaming/bidirectional_streaming_example.py\""
            echo -e "${GREEN}✓ Opened 3 terminal windows${NC}"
        elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
            # Linux - try different terminal emulators
            if command -v gnome-terminal &> /dev/null; then
                gnome-terminal -- bash -c "cd $(pwd) && .venv/bin/python -u examples/python/streaming/server_streaming_example.py; exec bash" &
                gnome-terminal -- bash -c "cd $(pwd) && .venv/bin/python -u examples/python/streaming/client_streaming_example.py; exec bash" &
                gnome-terminal -- bash -c "cd $(pwd) && .venv/bin/python -u examples/python/streaming/bidirectional_streaming_example.py; exec bash" &
                echo -e "${GREEN}✓ Opened 3 terminal windows${NC}"
            elif command -v xterm &> /dev/null; then
                xterm -e "cd $(pwd) && .venv/bin/python -u examples/python/streaming/server_streaming_example.py" &
                xterm -e "cd $(pwd) && .venv/bin/python -u examples/python/streaming/client_streaming_example.py" &
                xterm -e "cd $(pwd) && .venv/bin/python -u examples/python/streaming/bidirectional_streaming_example.py" &
                echo -e "${GREEN}✓ Opened 3 terminal windows${NC}"
            else
                echo -e "${RED}No supported terminal emulator found${NC}"
                echo -e "${YELLOW}Please run the examples manually in separate terminals${NC}"
            fi
        else
            echo -e "${RED}OS not supported for automatic terminal launching${NC}"
            echo -e "${YELLOW}Please run the examples manually in separate terminals:${NC}"
            echo ""
            echo "  Terminal 1: .venv/bin/python -u examples/python/streaming/server_streaming_example.py"
            echo "  Terminal 2: .venv/bin/python -u examples/python/streaming/client_streaming_example.py"
            echo "  Terminal 3: .venv/bin/python -u examples/python/streaming/bidirectional_streaming_example.py"
        fi
    fi
}

# Main menu loop
while true; do
    show_menu
    read -p "Select an option (0-4): " choice
    echo ""

    case $choice in
        1)
            run_example "server_streaming_example.py" "Server Streaming Example" "9001"
            ;;
        2)
            run_example "client_streaming_example.py" "Client Streaming Example" "9002"
            ;;
        3)
            run_example "bidirectional_streaming_example.py" "Bidirectional Streaming Example" "9003"
            ;;
        4)
            run_all
            break
            ;;
        0)
            echo -e "${GREEN}Goodbye!${NC}"
            exit 0
            ;;
        *)
            echo -e "${RED}Invalid option. Please try again.${NC}"
            echo ""
            ;;
    esac
done
