# Introduction

> **Version**: 0.2.0 | **Performance**: 172K+ RPS | **Features**: Cluster Management, Streaming, Code Generation

RpcNet is a high-performance QUIC-based RPC library built on `s2n-quic`. The library provides
high-level server and client primitives, TLS configuration helpers, rich support for
unary and streaming request flows, and complete distributed cluster management. This book
centralizes the user-facing materials so you can learn RpcNet in one place.

## Key Capabilities

### Core RPC
- TLS-first configuration for both client and server components
- Simple registration of request handlers with async closures
- Bidirectional, client-streaming, and server-streaming support
- Structured error reporting through `RpcError`
- Test-friendly abstractions that allow mocking QUIC streams

### Distributed Systems (v0.2.0+)
- **Cluster Management**: Built-in gossip protocol (SWIM) for node discovery
- **Load Balancing**: Multiple strategies (Round Robin, Random, Least Connections)
- **Health Checking**: Phi Accrual failure detection
- **Connection Pooling**: Efficient connection reuse
- **Tag-Based Routing**: Route requests by worker capabilities
- **Auto-Failover**: Zero-downtime worker replacement

### Performance
- **172K+ RPS**: Exceptional throughput with full QUIC+TLS encryption
- **Sub-millisecond latency**: < 0.1ms overhead
- **10K+ concurrent streams**: Per connection

## How To Read This Book

1. **Getting Started** walks through installing RpcNet and creating your first service.
2. **Core Concepts** introduces the configuration model, error types, and runtime fundamentals.
3. **Cluster Example** demonstrates building distributed systems with automatic discovery and load balancing.
4. **Streaming Patterns** covers bidirectional and one-way streaming.
5. **rpcnet-gen CLI** explains the code generation tool and workflows.

Throughout the chapters you will find executable snippets based on the working examples
in the repository.
