# Introduction

RpcNet is a QUIC-based RPC transport built on top of `s2n-quic`. The crate provides
high-level server and client primitives, TLS configuration helpers, and rich support
for unary and streaming request flows. This book centralises the user-facing
materials that previously lived in API comments so you can read them in one place
while keeping the library sources compact.

## Key Capabilities

- TLS-first configuration for both client and server components
- Simple registration of request handlers with async closures
- Bidirectional, client-streaming, and server-streaming helpers
- Structured error reporting through `RpcError`
- Test-friendly abstractions that allow mocking QUIC streams and connections

## How To Read This Book

1. **Core Concepts** introduces the configuration model and error types.
2. **Server Guide** walks through binding, registering methods, and starting the
   accept loop.
3. **Client Guide** covers connection setup plus simple RPC calls.
4. **Streaming Patterns** dives into bidirectional and one-way streaming helpers.
5. **Testing & Coverage** documents the built-in mocks used throughout our test
   suite and in the examples you can run locally.
6. **Reference Tables** summarise the main APIs, argument expectations, and
   return values at a glance.

Throughout the chapters you will find executable snippets based on the unit
and integration tests that accompany the crate.
