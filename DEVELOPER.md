# Developer Guide

This guide contains information for developers working on RpcNet itself.

## Table of Contents

- [Development Setup](#development-setup)
- [Running Tests](#running-tests)
- [Publishing to Crates.io](#publishing-to-cratesio)
- [Release Process](#release-process)

## Development Setup

### Prerequisites

- Rust 1.70 or later
- OpenSSL development libraries
- Git

### Clone and Build

```bash
# Clone the repository
git clone https://github.com/jsam/rpcnet.git
cd rpcnet

# Build the project
cargo build

# Build with all features
cargo build --all-features

# Build the CLI tool
cargo build --features codegen
```

### Generate Test Certificates

```bash
mkdir -p certs
cd certs

# Generate self-signed certificate for testing
openssl req -x509 -newkey rsa:4096 -keyout test_key.pem -out test_cert.pem -days 365 -nodes \
  -subj "/CN=localhost"

cd ..
```

## Running Tests

### Unit and Integration Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run cluster tests
cargo test --test cluster_integration
```

### Coverage

```bash
# Generate coverage report
make coverage

# Check coverage meets 90% threshold
make coverage-check

# Analyze coverage gaps
make coverage-gaps
```

### Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run with performance optimizations (jemalloc)
cargo bench --features perf

# Run specific benchmark
cargo bench --bench simple
```

## Publishing to Crates.io

### Prerequisites

1. **Crates.io Account**: You need a crates.io account and must be added as an owner of the `rpcnet` crate.

2. **Login to Crates.io**:
   ```bash
   cargo login
   ```
   Enter your API token when prompted. You can get your token from https://crates.io/me

3. **Verify Ownership**:
   ```bash
   cargo owner --list rpcnet
   ```

### Pre-Publication Checklist

Before publishing, ensure:

- [ ] All tests pass: `cargo test`
- [ ] All features build: `cargo build --all-features`
- [ ] Documentation builds: `cargo doc --all-features --no-deps`
- [ ] Coverage meets threshold: `make coverage-check`
- [ ] Benchmarks run successfully: `cargo bench`
- [ ] CHANGELOG.md is updated with release notes
- [ ] Version in `Cargo.toml` is incremented appropriately
- [ ] README.md is up to date
- [ ] All examples work correctly
- [ ] No uncommitted changes: `git status`

### Version Numbering

RpcNet follows [Semantic Versioning](https://semver.org/):

- **MAJOR** version (x.0.0): Incompatible API changes
- **MINOR** version (0.x.0): New functionality, backwards compatible
- **PATCH** version (0.0.x): Bug fixes, backwards compatible

### Publishing Steps

#### 1. Update Version

Edit `Cargo.toml` and update the version:

```toml
[package]
name = "rpcnet"
version = "0.3.0"  # Update this
```

#### 2. Update CHANGELOG

Add release notes to `CHANGELOG.md`:

```markdown
## [0.3.0] - 2025-01-15

### Added
- Cluster management with gossip protocol
- Load balancing strategies
- Phi Accrual failure detection

### Changed
- Improved performance to 172K+ RPS

### Fixed
- Bug fixes in connection handling
```

#### 3. Commit Version Bump

```bash
git add Cargo.toml CHANGELOG.md
git commit -m "chore: bump version to 0.3.0"
```

#### 4. Create Git Tag

```bash
git tag -a v0.3.0 -m "Release version 0.3.0"
```

#### 5. Review Package Contents

Check what files will be included in the package:

```bash
cargo package --list
```

**Note**: This will fail if you have uncommitted changes. Either commit them first or use `--allow-dirty` flag.

Review the output carefully to ensure:
- All necessary source files are included
- LICENSE files are present
- README.md and documentation are included
- Examples are packaged
- No sensitive files are accidentally included

#### 6. Test the Package Build

Build the package locally to verify it compiles correctly:

```bash
# Package the crate (creates a .crate file in target/package/)
cargo package --allow-dirty

# Test that the packaged crate builds
cd target/package
tar xvzf rpcnet-0.3.0.crate
cd rpcnet-0.3.0
cargo build --all-features
cargo test --lib
cd ../../..
```

This ensures the packaged version will build correctly on crates.io.

#### 7. Publish to Crates.io

Once you've verified everything:

```bash
cargo publish
```

**Important Notes**:
- This action **cannot be undone**! Once published, a version cannot be deleted (only yanked)
- The package must build successfully on crates.io's infrastructure
- All dependencies must already be published on crates.io
- The CLI tool (`rpcnet-gen`) will be automatically available to users who install the library

#### 8. Push Changes and Tags

After successful publication to crates.io:

```bash
git push origin main
git push origin v0.3.0
```

#### 9. Create GitHub Release

1. Go to https://github.com/jsam/rpcnet/releases/new
2. Select the tag you just created (`v0.3.0`)
3. Title: `v0.3.0`
4. Description: Copy the relevant section from CHANGELOG.md
5. Click "Publish release"

### Verify Publication

After publishing:

1. **Check crates.io**: Visit https://crates.io/crates/rpcnet
   - Verify version number is correct
   - Check that description and metadata are displayed properly
   - Confirm license information is shown

2. **Test CLI installation**:
   ```bash
   # Install the library (includes rpcnet-gen CLI by default)
   cargo install rpcnet
   
   # Verify CLI is available
   rpcnet-gen --help
   
   # Test code generation
   rpcnet-gen --input examples/basic_greeting/greeting.rpc.rs --output /tmp/test_gen
   ```

3. **Test library usage**:
   Create a test project to verify the library works:
   ```bash
   cargo new test-rpcnet
   cd test-rpcnet
   
   # Add to Cargo.toml:
   # [dependencies]
   # rpcnet = "0.3.0"
   
   cargo build
   ```

4. **Check documentation**: Visit https://docs.rs/rpcnet
   - Verify all modules are documented
   - Check that examples render correctly
   - Ensure cluster module documentation is present

5. **Monitor build status**: 
   - docs.rs builds documentation automatically
   - Check https://docs.rs/crate/rpcnet/0.3.0/builds for build status
   - If the build fails, investigate and potentially yank the version

### Yanking a Release

If you discover a critical bug after publishing:

```bash
# Yank a specific version (prevents new projects from using it)
cargo yank --vers 0.3.0

# Un-yank if needed
cargo yank --vers 0.3.0 --undo
```

**Note**: Yanking does not delete the version, it just prevents new projects from depending on it.

## Release Process Summary

### Option 1: Using Makefile (Recommended)

The Makefile provides convenient targets that handle all checks automatically:

```bash
# 1. Update version and documentation
vim Cargo.toml          # Change version = "0.3.0"
vim CHANGELOG.md        # Add release notes
vim README.md           # Update version numbers if needed

# 2. Commit changes
git add Cargo.toml CHANGELOG.md README.md
git commit -m "chore: bump version to 0.3.0"
git tag -a v0.3.0 -m "Release version 0.3.0"

# 3. Run pre-publication checks
make publish-check      # Runs tests, lint, format check, docs build

# 4. Dry run (optional but recommended)
make publish-dry-run    # Packages and verifies contents

# 5. Publish to crates.io
make publish            # Runs checks, asks for confirmation, then publishes

# 6. Push to GitHub
git push origin main
git push origin v0.3.0

# 7. Create GitHub release
# Visit: https://github.com/jsam/rpcnet/releases/new
# - Select tag: v0.3.0
# - Title: v0.3.0
# - Description: Copy from CHANGELOG.md
# - Publish release

# 8. Verify publication
# - Check https://crates.io/crates/rpcnet
# - Check https://docs.rs/rpcnet
# - Test: cargo install rpcnet && rpcnet-gen --help
```

### Option 2: Manual Process

Complete checklist for releasing a new version:

```bash
# 1. Update version in Cargo.toml
vim Cargo.toml
# Change version = "0.3.0"

# 2. Update CHANGELOG.md
vim CHANGELOG.md
# Add new release section with changes

# 3. Update README.md if needed
vim README.md
# Update version numbers in installation examples

# 4. Run full test suite
cargo test --all-features
make coverage-check
cargo bench

# 5. Verify CLI works
cargo build --all-features
./target/debug/rpcnet-gen --help

# 6. Commit and tag
git add Cargo.toml CHANGELOG.md README.md
git commit -m "chore: bump version to 0.3.0"
git tag -a v0.3.0 -m "Release version 0.3.0"

# 7. Review package contents
cargo package --list --allow-dirty

# 8. Test package build
cargo package --allow-dirty
cd target/package
tar xvzf rpcnet-0.3.0.crate
cd rpcnet-0.3.0
cargo build --all-features
cargo test --lib
cd ../../..

# 9. Publish to crates.io
cargo publish

# 10. Push to GitHub
git push origin main
git push origin v0.3.0

# 11. Create GitHub release
# Go to: https://github.com/jsam/rpcnet/releases/new
# - Select tag: v0.3.0
# - Title: v0.3.0
# - Description: Copy from CHANGELOG.md
# - Publish release

# 12. Verify publication
# - Check https://crates.io/crates/rpcnet
# - Check https://docs.rs/rpcnet
# - Test: cargo install rpcnet && rpcnet-gen --help

# 13. Announce (optional)
# - Post to Reddit r/rust
# - Submit to This Week in Rust
# - Tweet/social media
```

### Makefile Targets

The project includes helpful Makefile targets for the release process:

- **`make publish-check`**: Runs all pre-publication checks
  - Tests (all features)
  - Code formatting verification
  - Linter (clippy)
  - Documentation build
  - Coverage check (warning only)

- **`make publish-dry-run`**: Packages the crate and shows contents
  - Runs `publish-check` first
  - Lists files that will be published
  - Creates package in `target/package/`
  - Does NOT publish to crates.io

- **`make publish`**: Publishes to crates.io
  - Runs `publish-check` first
  - Asks for confirmation (type "yes")
  - Publishes to crates.io
  - Shows next steps after successful publication

**Recommended workflow**: Always run `make publish-dry-run` before `make publish` to verify the package contents.

## Troubleshooting

### Publication Fails

**Issue**: `error: failed to publish`

**Solutions**:
- Ensure you're logged in: `cargo login`
- Check you're an owner: `cargo owner --list rpcnet`
- Verify version doesn't already exist on crates.io
- Ensure all dependencies are published on crates.io
- Check for uncommitted changes (commit or use `--allow-dirty`)
- Verify Cargo.toml has all required fields

**Issue**: `error: some crates failed to publish`

**Solutions**:
- Read the error message carefully
- Common causes:
  - Version already exists
  - Missing license files
  - Invalid metadata in Cargo.toml
  - Build failure on crates.io's servers

### Documentation Build Fails

**Issue**: `cargo doc` fails locally

**Solutions**:
- Fix any broken doc links
- Ensure all code examples in docs compile
- Run `cargo doc --all-features --no-deps` to check
- Use `#[doc(hidden)]` for internal items if needed

**Issue**: docs.rs build fails after publication

**Solutions**:
- Check https://docs.rs/crate/rpcnet/VERSION/builds
- Common issues:
  - Feature combinations not working
  - Platform-specific code issues
  - Missing documentation for public items
- If unfixable, consider yanking and republishing with fix

### Package is Too Large

**Issue**: `error: package is too large`

**Solutions**:
- Add files to `.gitignore` that shouldn't be packaged
- Use `exclude` in Cargo.toml:
  ```toml
  [package]
  exclude = [
    "docs/mdbook/book/*",
    "target/*",
    "*.coverage",
  ]
  ```
- Check with `cargo package --list` what's being included

### CLI Not Found After Installation

**Issue**: `rpcnet-gen: command not found` after `cargo install rpcnet`

**Solutions**:
- Verify cargo bin directory is in PATH: `echo $PATH | grep cargo`
- Add to PATH: `export PATH="$HOME/.cargo/bin:$PATH"`
- Reinstall: `cargo install rpcnet --force`
- Check installation: `ls ~/.cargo/bin/rpcnet-gen`

### Tests Fail on CI

**Issue**: Tests pass locally but fail on CI

**Solutions**:
- Check certificate generation in CI
- Verify all features are tested
- Review CI logs for environment differences
- Ensure timeout values work in CI environment

### Version Already Published

**Issue**: Accidentally published wrong version

**Solutions**:
- You **cannot** delete a published version
- Options:
  1. Yank the version: `cargo yank --vers 0.3.0`
  2. Publish a patch version with fix: `0.3.1`
  3. If critical security issue, publish `0.3.1` and yank `0.3.0`
- Remember: Yanking doesn't delete, just prevents new usage

## Important Notes

### About the CLI Tool

Starting with version 0.2.0, the `rpcnet-gen` CLI tool is **included by default** when installing the library:

```bash
# This installs both the library AND the CLI tool
cargo install rpcnet
```

**No feature flags needed!** Users previously had to use `--features codegen`, but this is no longer necessary.

When you publish, both the library and the `rpcnet-gen` binary will be available to users automatically.

### Cargo.toml Configuration

The key configuration that makes the CLI available by default:

```toml
[[bin]]
name = "rpcnet-gen"
path = "src/bin/rpcnet-gen.rs"
# No required-features! Available by default.

[features]
default = ["codegen", "perf"]  # codegen is in default features
codegen = []  # Empty feature, but dependencies always included

# These are now always included (not optional):
[dependencies]
syn = { version = "2.0", features = ["full", "extra-traits", "parsing"] }
quote = { version = "1.0" }
proc-macro2 = { version = "1.0" }
clap = { version = "4.0", features = ["derive"] }
prettyplease = { version = "0.2" }
```

### What Users Get

When someone runs `cargo install rpcnet`, they get:

1. **The Library**: All RPC functionality, cluster management, etc.
2. **The CLI Tool**: `rpcnet-gen` for code generation
3. **Default Features**: Performance optimizations (jemalloc allocator)

They can then:
- Use the library in their `Cargo.toml`
- Run `rpcnet-gen` from command line to generate code
- No additional setup needed!

## Getting Help

- **Issues**: https://github.com/jsam/rpcnet/issues
- **Discussions**: https://github.com/jsam/rpcnet/discussions
- **Email**: contact@justsam.io
- **Crates.io**: https://crates.io/crates/rpcnet
- **Documentation**: https://docs.rs/rpcnet
