# rpcnet-gen CLI

The `rpcnet-gen` binary turns a Rust service definition (`*.rpc.rs`) into the
client, server, and type modules consumed by your application. This chapter
covers installation, day-to-day usage, and automation patterns.

## Installing

Starting with v0.2.0, the CLI is included by default with rpcnet. Install it once and reuse it across workspaces:

```bash
cargo install rpcnet
```

The CLI is always available - no feature flags needed! This is a major usability improvement over v0.1.x.

Add `--locked` in CI to guarantee reproducible dependency resolution.

## Input Files at a Glance

Service definitions are ordinary Rust modules annotated with `#[rpcnet::service]`.
For example:

```rust
// src/greeting.rpc.rs
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GreetRequest {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GreetResponse {
    pub message: String,
}

#[rpcnet::service]
pub trait Greeting {
    async fn greet(&self, request: GreetRequest) -> Result<GreetResponse, GreetingError>;
}
```

Every request/response/error type must be `Serialize`/`Deserialize`, and all
trait methods must be `async fn` returning `Result<T, E>`.

## Basic Invocation

Run the generator whenever you change a service trait:

```bash
rpcnet-gen --input src/greeting.rpc.rs --output src/generated
```

A successful run prints the generated paths and writes the following structure:

```
src/generated/
└── greeting/
    ├── client.rs   # GreetingClient with typed async methods
    ├── mod.rs      # Module exports and re-exports
    ├── server.rs   # GreetingServer + GreetingHandler trait
    └── types.rs    # Request/response/error definitions
```

Import the module once and re-export whatever you need:

```rust
#[path = "generated/greeting/mod.rs"]
mod greeting;

use greeting::{client::GreetingClient, server::{GreetingHandler, GreetingServer}};
```

## Command-Line Options

`rpcnet-gen --help` surfaces all switches:

```
Generate RPC client and server code from service definitions

Usage: rpcnet-gen [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>    Input .rpc file (Rust source with service trait)
  -o, --output <OUTPUT>  Output directory for generated code [default: src/generated]
      --server-only      Generate only server code
      --client-only      Generate only client code
      --types-only       Generate only type definitions
  -h, --help             Print help
  -V, --version          Print version
```

Key behaviours:

- Omit `--output` to use `src/generated`. The generator creates a lowercase
  subdirectory named after the service (`Greeting` → `greeting/`).
- Combine `--server-only`, `--client-only`, and `--types-only` to tailor the
  outputs. The implicit `mod.rs` only re-exports files that were produced.
- Passing mutually exclusive flags (e.g. `--server-only --client-only`) produces
  only the directories you asked for; `types.rs` is skipped when either flag is
  present.

## Regenerating Automatically

### Manual rebuilds

Run the command by hand after touching a `.rpc.rs` file. Consider wiring a
`cargo alias` or a shell script so teammates can regenerate with a single
command.

### With `cargo watch`

Install `cargo-watch` and keep generated code up to date during development:

```bash
cargo install cargo-watch
cargo watch -w src/greeting.rpc.rs -x "run --bin rpcnet-gen -- --input src/greeting.rpc.rs --output src/generated"
```

### Through `build.rs`

For projects that must guarantee generated code exists before compilation,
invoke the builder API from a build script (requires the `codegen` feature in
`[build-dependencies]`):

```rust
// build.rs
fn main() {
    println!("cargo:rerun-if-changed=src/greeting.rpc.rs");

    rpcnet::codegen::Builder::new()
        .input("src/greeting.rpc.rs")
        .output("src/generated")
        .build()
        .expect("Failed to generate RPC code");
}
```

Cargo reruns the script when the `.rpc.rs` file changes, keeping the generated
modules in sync.

## Working With Multiple Services

Generate several services in one go by running the CLI multiple times or by
stacking inputs in the builder:

```rust
// build.rs
fn main() {
    for service in ["rpc/user.rpc.rs", "rpc/billing.rpc.rs", "rpc/audit.rpc.rs"] {
        println!("cargo:rerun-if-changed={service}");
    }

    rpcnet::codegen::Builder::new()
        .input("rpc/user.rpc.rs")
        .input("rpc/billing.rpc.rs")
        .input("rpc/audit.rpc.rs")
        .output("src/generated")
        .build()
        .expect("Failed to generate RPC code");
}
```

Each input produces a sibling directory under `src/generated/` (`user/`,
`billing/`, `audit/`).

## Version-Control Strategy

Generated code is ordinary Rust and can be committed. Most teams either:

1. Commit the `src/generated/**` tree so downstream crates build without the
   generator, or
2. Ignore the directory and require the CLI (or `build.rs`) to run during CI.

Pick a single approach and document it for contributors.

## Troubleshooting

- **Missing input file** – the CLI exits with `Error: Input file '…' does not
  exist`. Double-check the path and ensure the file is tracked in git so
  collaborators receive it.
- **Invalid trait** – methods must be `async fn` and return `Result`. The parser
  reports an error pointing at the offending signature.
- **Serialization failures at runtime** – make sure your request/response/error
  types derive `Serialize` and `Deserialize` and keep both client and server on
  the same crate version so layouts match.

With these workflows in place you can treat `rpcnet-gen` like any other build
step: edit the `.rpc.rs` trait, regenerate, and keep building.
