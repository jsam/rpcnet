# Environment Variables Reference

This document lists all environment variables used in the connection_swap example with the dual-port architecture.

## Director

The director runs two servers:
- **USER Port (61000)**: Accepts client `generate` streaming requests
- **MGMT Port (61001)**: Handles worker registration and health checks

```bash
CONNECTION_SWAP_DIRECTOR_USER_ADDR=127.0.0.1:61000 \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --manifest-path examples/connection_swap/Cargo.toml --bin director
```

### Variables:
- `CONNECTION_SWAP_DIRECTOR_USER_ADDR` - Address for client connections (default: `127.0.0.1:61000`)
- `CONNECTION_SWAP_DIRECTOR_MGMT_ADDR` - Address for worker management (default: `127.0.0.1:61001`)
- `RUST_LOG` - Log level (e.g., `info`, `debug`, `warn`)

## Worker

Each worker runs two servers:
- **USER Port**: Exposes `generate` streaming endpoint
- **MGMT Port**: Exposes `health_check` endpoint

### Worker A:
```bash
CONNECTION_SWAP_WORKER_USER_ADDR=127.0.0.1:62001 \
CONNECTION_SWAP_WORKER_MGMT_ADDR=127.0.0.1:63001 \
CONNECTION_SWAP_WORKER_LABEL=worker-a \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --manifest-path examples/connection_swap/Cargo.toml --bin worker
```

### Worker B:
```bash
CONNECTION_SWAP_WORKER_USER_ADDR=127.0.0.1:62002 \
CONNECTION_SWAP_WORKER_MGMT_ADDR=127.0.0.1:63002 \
CONNECTION_SWAP_WORKER_LABEL=worker-b \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --manifest-path examples/connection_swap/Cargo.toml --bin worker
```

### Variables:
- `CONNECTION_SWAP_WORKER_USER_ADDR` - Address for RPC endpoints (default: `127.0.0.1:62001`)
- `CONNECTION_SWAP_WORKER_MGMT_ADDR` - Address for management endpoints (default: `127.0.0.1:63001`)
- `CONNECTION_SWAP_WORKER_LABEL` - Unique worker identifier (e.g., `worker-a`, `worker-b`)
- `CONNECTION_SWAP_DIRECTOR_MGMT_ADDR` - Director's management port to connect to (default: `127.0.0.1:61001`)
- `RUST_LOG` - Log level

## Client

The client connects to the director's USER port:

```bash
CONNECTION_SWAP_DIRECTOR_TARGET=127.0.0.1:61000 \
RUST_LOG=info \
    cargo run --manifest-path examples/connection_swap/Cargo.toml --bin client
```

### Variables:
- `CONNECTION_SWAP_DIRECTOR_TARGET` - Director's USER port (default: `127.0.0.1:61000`)
- `RUST_LOG` - Log level

## Port Allocation Summary

| Component   | USER Port | MGMT Port |
|-------------|-----------|-----------|
| Director    | 61000     | 61001     |
| Worker A    | 62001     | 63001     |
| Worker B    | 62002     | 63002     |
| Client      | connects to 61000 | - |

## Complete Setup (4 Terminals)

### Terminal 1 - Director
```bash
CONNECTION_SWAP_DIRECTOR_USER_ADDR=127.0.0.1:61000 \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --manifest-path examples/connection_swap/Cargo.toml --bin director
```

### Terminal 2 - Worker A
```bash
CONNECTION_SWAP_WORKER_USER_ADDR=127.0.0.1:62001 \
CONNECTION_SWAP_WORKER_MGMT_ADDR=127.0.0.1:63001 \
CONNECTION_SWAP_WORKER_LABEL=worker-a \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --manifest-path examples/connection_swap/Cargo.toml --bin worker
```

### Terminal 3 - Worker B
```bash
CONNECTION_SWAP_WORKER_USER_ADDR=127.0.0.1:62002 \
CONNECTION_SWAP_WORKER_MGMT_ADDR=127.0.0.1:63002 \
CONNECTION_SWAP_WORKER_LABEL=worker-b \
CONNECTION_SWAP_DIRECTOR_MGMT_ADDR=127.0.0.1:61001 \
RUST_LOG=info \
    cargo run --manifest-path examples/connection_swap/Cargo.toml --bin worker
```

### Terminal 4 - Client
```bash
CONNECTION_SWAP_DIRECTOR_TARGET=127.0.0.1:61000 \
RUST_LOG=info \
    cargo run --manifest-path examples/connection_swap/Cargo.toml --bin client
```

## Automated Demo

The `run_demo.sh` script sets all these variables automatically:

```bash
cd examples/connection_swap
./run_demo.sh
```
