# Publishing RpcNet with Makefile

Quick reference guide for publishing RpcNet to crates.io using Makefile targets.

## Quick Start

```bash
# 1. Update version and docs
vim Cargo.toml CHANGELOG.md README.md

# 2. Commit and tag
git add -A
git commit -m "chore: bump version to 0.2.0"
git tag -a v0.2.0 -m "Release version 0.2.0"

# 3. Check everything is ready
make publish-check

# 4. Dry run (see what will be published)
make publish-dry-run

# 5. Publish!
make publish
```

## Makefile Targets

### `make publish-check`

Runs all pre-publication checks to ensure the package is ready:

✅ Tests (all features)  
✅ Code formatting  
✅ Linter (clippy with warnings as errors)  
✅ Documentation build  
⚠️  Coverage check (warning only)

**When to use**: Before making any publication attempt, or during development to ensure quality.

**Example output**:
```
=== Pre-Publication Checks ===

1. Verifying all tests pass...
✅ All tests passed

2. Checking code formatting...
✅ Code is properly formatted

3. Running linter...
✅ No clippy warnings

4. Building documentation...
✅ Documentation builds successfully

5. Checking coverage...
✅ Coverage 92.3% meets threshold

✅ All pre-publication checks passed!
```

### `make publish-dry-run`

Packages the crate and shows what files will be published:

1. Runs `publish-check` first
2. Lists all files that will be included
3. Creates package in `target/package/`
4. **Does NOT publish** to crates.io

**When to use**: Before actual publication to verify package contents.

**Example output**:
```
=== Packaging rpcnet (library + rpcnet-gen CLI) ===

Reviewing package contents...
Cargo.toml
README.md
LICENSE-MIT
LICENSE-APACHE
src/lib.rs
src/cluster/mod.rs
...

Building package...
✅ Package created successfully!

Package location: target/package/rpcnet-0.2.0.crate

To publish, run: make publish
```

### `make publish`

Publishes to crates.io with safety confirmation:

1. Runs `publish-check` first
2. Asks for confirmation (you must type "yes")
3. Publishes to crates.io
4. Shows next steps after success

**When to use**: When you're ready to publish and have verified everything.

**Example output**:
```
=== Publishing to crates.io ===

⚠️  WARNING: This action cannot be undone!

Are you sure you want to publish rpcnet v0.2.0? (yes/no): yes

Publishing rpcnet crate (library + rpcnet-gen CLI)...
    Uploading rpcnet v0.2.0
✅ Successfully published to crates.io!

Next steps:
  1. Push to GitHub: git push origin main
  2. Push tag: git push origin v0.2.0
  3. Create GitHub release: https://github.com/jsam/rpcnet/releases/new
  4. Verify on crates.io: https://crates.io/crates/rpcnet
  5. Check docs.rs: https://docs.rs/rpcnet
```

## Complete Workflow

### Recommended: Safe and Verified

```bash
# Step 1: Prepare release
vim Cargo.toml          # Update version
vim CHANGELOG.md        # Add release notes
vim README.md           # Update if needed

# Step 2: Commit changes
git add Cargo.toml CHANGELOG.md README.md
git commit -m "chore: prepare v0.2.0 release"
git tag -a v0.2.0 -m "Release version 0.2.0"

# Step 3: Pre-publication checks
make publish-check
# Wait for all checks to pass

# Step 4: Verify package contents (IMPORTANT!)
make publish-dry-run
# Review the file list carefully

# Step 5: Publish
make publish
# Type "yes" when prompted

# Step 6: Push to GitHub
git push origin main
git push origin v0.2.0

# Step 7: Verify publication
# - Visit https://crates.io/crates/rpcnet
# - Test: cargo install rpcnet && rpcnet-gen --help
# - Check https://docs.rs/rpcnet/0.2.0
```

### Quick: For Experienced Publishers

```bash
vim Cargo.toml CHANGELOG.md
git add -A && git commit -m "chore: bump v0.2.0" && git tag -a v0.2.0 -m "v0.2.0"
make publish  # Runs checks automatically
git push origin main && git push origin v0.2.0
```

## What Gets Checked

The `publish-check` target verifies:

### 1. Tests (Critical)
```bash
cargo test --all-features
```
- All 183+ tests must pass
- Tests both library and integration tests
- Includes cluster and streaming tests

### 2. Code Formatting (Critical)
```bash
cargo fmt --check
```
- Code must follow Rust formatting standards
- Run `make format` if this fails

### 3. Linter (Critical)
```bash
cargo clippy --all-targets --all-features -- -D warnings
```
- No clippy warnings allowed
- Treats warnings as errors
- Ensures code quality

### 4. Documentation (Critical)
```bash
cargo doc --no-deps --all-features
```
- Documentation must build without errors
- Ensures docs.rs will build successfully

### 5. Coverage (Warning Only)
```bash
make coverage-check
```
- Checks for 90%+ coverage
- Only warns if it fails (doesn't block)

## Troubleshooting

### Tests Fail
```bash
# Run tests to see failures
cargo test --all-features

# Fix the failing tests
vim src/...

# Re-run checks
make publish-check
```

### Formatting Issues
```bash
# Auto-format code
make format

# Or manually
cargo fmt

# Verify
make publish-check
```

### Clippy Warnings
```bash
# See warnings
cargo clippy --all-targets --all-features

# Fix warnings
vim src/...

# Re-check
make publish-check
```

### Documentation Errors
```bash
# Build docs to see errors
cargo doc --all-features

# Fix doc comments
vim src/...

# Verify
make publish-check
```

### Cancel Publication

If you accidentally start `make publish`:

1. **Before typing "yes"**: Just type "no" or press Ctrl+C
2. **After typing "yes"**: Cannot cancel - publish completes immediately
3. **After published**: Cannot delete - can only yank version

## Safety Features

The Makefile publish targets include multiple safety features:

✅ **Automatic checks**: Runs tests, lint, format before publishing  
✅ **Confirmation prompt**: Must type "yes" to proceed  
✅ **Dry run option**: `publish-dry-run` lets you verify first  
✅ **Clear output**: Shows what's happening at each step  
✅ **Next steps**: Reminds you to push tags and create release  
⚠️ **No undo warning**: Clearly states action cannot be undone

## Files Published

When you run `make publish`, these files are included:

**Core Library**:
- `src/**/*.rs` - All source code
- `Cargo.toml` - Package metadata
- `LICENSE-MIT`, `LICENSE-APACHE` - Licenses

**Documentation**:
- `README.md` - Project overview
- `DEVELOPER.md` - Development guide
- `TESTING.md` - Testing guide

**Examples**:
- `examples/**/*` - All examples including cluster
- `examples/cluster/README.md` - Cluster setup guide

**Tests & Benchmarks**:
- `tests/**/*.rs` - Test suites
- `benches/**/*.rs` - Benchmarks

**Certificates**:
- `certs/test_*.pem` - Test certificates (dev only)

## CLI Tool Included

**Important**: Starting with v0.2.0, the `rpcnet-gen` CLI is included by default!

Users get both:
1. The library (via `Cargo.toml` dependency)
2. The CLI tool (via `cargo install rpcnet`)

No feature flags needed - it just works! 🎉

## Post-Publication

After `make publish` succeeds:

### Immediate (Required)
```bash
git push origin main
git push origin v0.2.0
```

### Create GitHub Release (Required)
1. Go to https://github.com/jsam/rpcnet/releases/new
2. Select tag `v0.2.0`
3. Title: `v0.2.0`
4. Copy release notes from CHANGELOG.md
5. Publish release

### Verify (Recommended)
1. Check crates.io: https://crates.io/crates/rpcnet
2. Wait for docs.rs: https://docs.rs/rpcnet/0.2.0
3. Test installation:
   ```bash
   cargo install rpcnet
   rpcnet-gen --help
   ```

### Announce (Optional)
- Post to Reddit r/rust
- Submit to This Week in Rust
- Tweet/social media
- Blog post

## Additional Commands

Other useful Makefile targets:

```bash
make help           # Show all available targets
make test           # Run all tests
make lint           # Run clippy
make format         # Format code
make doc            # Build and open docs
make bench          # Run benchmarks
make clean          # Clean build artifacts
```

---

**Quick Reference**: `make publish-check` → `make publish-dry-run` → `make publish`
