# Development Guide

Quick reference for common development tasks.

## Quick Start

```bash
# Run tests with linting (recommended for local development)
make test

# Just run tests (faster, skip linting)
make test-quick

# Run everything before committing
make pre-commit
```

## Testing

### Basic Testing

```bash
# Run all tests + quick lint (recommended)
make test

# Run tests only (faster)
make test-quick
# or
make test-only

# Run unit tests only
make test-unit

# Run integration tests only
make test-integration
```

### What `make test` does:
1. Runs all tests
2. Runs clippy on lib + bins (catches most issues)
3. Shows a summary

**Time:** ~30-60 seconds (depending on cache)

## Pre-Commit Checks

Before committing your code:

```bash
make pre-commit
```

This runs:
1. ✅ Auto-formats code (`cargo fmt`)
2. ✅ Quick lint check (lib + bins)
3. ✅ Unit tests

**Time:** ~30-60 seconds

**Tip:** Run this before every commit to catch issues early!

## Linting

```bash
# Quick lint (lib + bins only, used by make test)
make lint-quick

# Full lint (all targets including tests, examples, benches)
make lint

# Auto-fix simple issues
cargo clippy --fix
```

### Why two lint commands?

- **`lint-quick`** - Fast, catches 95% of issues (used by `make test`)
- **`lint`** - Comprehensive, checks everything (used by CI)

## Formatting

```bash
# Format code
make format

# Check if formatted (doesn't modify files)
make format-check
```

**Tip:** Configure your editor to auto-format on save!

## Coverage

```bash
# Generate coverage report (HTML)
make coverage-html

# Check if coverage meets 65% threshold
make coverage-check

# Find uncovered lines
make coverage-gaps
```

## Documentation

```bash
# Build and open docs
make doc

# Build including private items
make doc-private

# Build user guide (mdbook)
make doc-book

# Serve user guide with live reload
make doc-book-serve
```

## Benchmarks

```bash
# Run all benchmarks
make bench
```

## Examples

```bash
# Build all examples
make examples
```

## Pre-Release Checks

Before creating a release:

```bash
# Run all publication checks
make publish-check

# Dry run (see what would be published)
make publish-dry-run
```

This runs:
1. All tests (all features)
2. Format check
3. Full clippy lint (all targets)
4. Documentation build

**Time:** ~5-10 minutes

## Continuous Development

Watch files and run tests on changes:

```bash
make dev-watch
```

Requires: `cargo install cargo-watch`

## Common Workflows

### Before Starting Work

```bash
git checkout main
git pull
cargo update
make test
```

### During Development

```bash
# Make changes...
make test          # Quick check after changes
# Make more changes...
make test          # Check again
```

### Before Committing

```bash
make pre-commit    # Auto-formats + checks everything
git add .
git commit -m "feat: add new feature"
```

### Before Pushing

```bash
# Optional: run full CI checks locally
make ci

# Or just the quick pre-commit
make pre-commit
git push
```

### Before Release

```bash
# Update version in Cargo.toml
make publish-check    # Full validation
make publish-dry-run  # See what will be published

# Create release
git tag v0.2.1
git push origin v0.2.1
# GitHub Actions handles the rest!
```

## CI Commands

These are used by GitHub Actions but can be run locally:

```bash
make ci           # Run all CI checks
make ci-test      # CI test suite
make ci-lint      # CI linting
make ci-coverage  # CI coverage
make ci-security  # Security audit
```

## Troubleshooting

### Tests failing?

```bash
# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run with backtrace
RUST_BACKTRACE=1 cargo test
```

### Clippy errors?

```bash
# Auto-fix simple issues
cargo clippy --fix

# See detailed explanation
cargo clippy -- -W clippy::all -W clippy::pedantic

# Allow specific lint (in code)
#[allow(clippy::lint_name)]
```

### Format issues?

```bash
# Format everything
make format

# Check what would change
cargo fmt -- --check
```

### Coverage too low?

```bash
# See coverage gaps
make coverage-gaps

# Generate detailed HTML report
make coverage-html
```

### Build issues?

```bash
# Clean everything
make clean

# Or more aggressive
cargo clean
rm -rf target/

# Rebuild
cargo build
```

## Tips & Best Practices

### 1. Run `make test` frequently
- Catches issues early
- Fast enough for frequent use (~30-60s)
- Includes linting

### 2. Use `make pre-commit` before committing
- Ensures code is formatted
- Catches clippy warnings
- Runs tests
- Takes ~30-60s

### 3. Let CI do the heavy lifting
- CI runs full checks on all platforms
- You don't need to run everything locally
- Focus on fast local iteration

### 4. Configure your editor
- Auto-format on save
- Show clippy warnings inline
- Run tests in the background

### 5. Use watch mode during development
```bash
make dev-watch
```

### 6. Commit often with good messages
```bash
git commit -m "type: brief description"
```

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`

## Speed Comparison

| Command | Time | When to Use |
|---------|------|-------------|
| `make test` | ~30-60s | After every change (recommended) |
| `make pre-commit` | ~30-60s | Before committing |
| `make lint` | ~2min | Occasionally, CI does this |
| `make publish-check` | ~5-10min | Before release |
| `make coverage-html` | ~10min | When checking coverage |

## Editor Setup

### VS Code

Install extensions:
- `rust-analyzer` - Code intelligence
- `Even Better TOML` - TOML syntax

Settings (`settings.json`):
```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
```

### Vim/Neovim

Use `rust.vim` or `nvim-lsp` with `rust-analyzer`.

### IntelliJ/CLion

Use the Rust plugin with built-in clippy and rustfmt integration.

## Getting Help

- Run `make help` to see all commands
- Check `Makefile` for command details
- See `.github/workflows/` for CI configuration
- Read `RELEASE.md` for release process
- Open an issue if you find bugs

## Environment Variables

```bash
# Enable backtraces
export RUST_BACKTRACE=1

# Verbose output
export RUST_LOG=debug

# Faster compilation (less optimization)
export CARGO_PROFILE_DEV_OPT_LEVEL=0
```

## Useful Cargo Commands

```bash
# Update dependencies
cargo update

# Check without building
cargo check

# Build release version
cargo build --release

# Run specific example
cargo run --example calculator_client

# Clean build artifacts
cargo clean

# Show dependency tree
cargo tree
```

---

Happy coding! 🦀
