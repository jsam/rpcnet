# GitHub Actions Workflows

This directory contains the CI/CD workflows for rpcnet.

## Workflows

### 🔍 PR Checks (`pr-checks.yml`)

Runs comprehensive checks on every pull request and push to main. This is the primary workflow that ensures code quality.

**Triggered on:**
- Pull requests to `main`
- Pushes to `main`

**Jobs:**

1. **Format Check** - Ensures code is properly formatted with `rustfmt`
2. **Clippy Lint** - Runs Clippy linter with strict warnings
3. **Test Suite** - Runs all tests on multiple platforms (Ubuntu, macOS) and Rust versions (stable, beta)
4. **Coverage Analysis** - Measures code coverage and enforces 90% threshold
5. **Security Audit** - Checks for security vulnerabilities with `cargo-audit`
6. **Documentation** - Builds docs and checks for broken links
7. **Examples** - Verifies all examples compile successfully

**Status:** All jobs must pass for PR to be mergeable.

### 📊 Coverage Report (`coverage.yml`)

Generates detailed coverage reports and uploads to Codecov.

**Triggered on:**
- Push to `main` branch
- Daily at 2 AM UTC (scheduled)
- Manual dispatch

**Features:**
- Uploads coverage to Codecov
- Generates HTML and JSON reports
- Comments coverage percentage on PRs
- Archives coverage artifacts

### 🚀 Release (`release.yml`)

Automates the complete release process to crates.io.

**Triggered on:**
- Push of version tags (e.g., `v0.2.1`)
- GitHub release creation
- Manual workflow dispatch

**Jobs:**

1. **Validation** - Runs all publish checks, verifies version matches tag
2. **Publish** - Publishes package to crates.io using `CARGO_REGISTRY_TOKEN`
3. **Build Artifacts** - Builds `rpcnet-gen` binaries for Linux and macOS (x86_64 and ARM64)
4. **Verify Published** - Confirms package is available on crates.io
5. **Announce** - Creates release summary with installation instructions

**Features:**
- Automatic version validation
- Multi-platform binary builds
- Package verification
- Dry-run support for testing
- Comprehensive release summaries

**See:** `RELEASE.md` for complete release instructions

## Local Testing

Before pushing, you can run the same checks locally using the Makefile:

```bash
# Run all CI checks
make ci

# Individual checks
make ci-lint        # Clippy linting
make ci-test        # Run tests
make ci-coverage    # Coverage analysis
make ci-security    # Security audit

# Pre-publish checks
make publish-check  # All checks before publishing
```

## Required Secrets

For full functionality, the following GitHub secrets should be configured:

- `CODECOV_TOKEN` - Token for uploading coverage to Codecov.io

## Badge Status

The README includes status badges for:
- PR Checks workflow status
- Code coverage percentage
- Crates.io version
- Documentation status
- License information

## Caching

All workflows use GitHub Actions cache to speed up builds:
- Cargo registry cache
- Cargo build cache
- Platform-specific caches

This significantly reduces CI run times.

## Matrix Testing

The test job runs on multiple configurations:
- **OS:** Ubuntu (Linux), macOS
- **Rust:** stable, beta
- Excludes: macOS + beta (to save CI minutes)

## Artifacts

Workflows generate and upload the following artifacts:
- **Coverage reports** (HTML, JSON, XML) - 30 day retention
- **Test results** - Available in job summaries

## Workflow Dependencies

All workflows use updated action versions:
- `actions/checkout@v4`
- `actions/cache@v4`
- `actions/upload-artifact@v4`
- `codecov/codecov-action@v4`
- `actions/github-script@v7`
- `dtolnay/rust-toolchain@stable`

## Certificate Generation

Tests require TLS certificates. All workflows automatically generate test certificates:

```bash
openssl req -x509 -newkey rsa:4096 \
  -keyout certs/test_key.pem \
  -out certs/test_cert.pem \
  -days 365 -nodes \
  -subj "/CN=localhost"
```

## Troubleshooting

### Tests failing locally but passing in CI

Ensure you have test certificates generated:
```bash
mkdir -p certs
openssl req -x509 -newkey rsa:4096 -keyout certs/test_key.pem -out certs/test_cert.pem -days 365 -nodes -subj "/CN=localhost"
```

### Coverage too low

Run coverage locally to see gaps:
```bash
make coverage
make coverage-gaps
```

### Format errors

Run format before committing:
```bash
make format
```

### Clippy errors

Fix lints before pushing:
```bash
make lint
```

## Performance

Typical workflow run times:
- **Format Check:** ~30 seconds
- **Lint:** ~2 minutes (with cache)
- **Tests:** ~5 minutes per platform
- **Coverage:** ~10 minutes
- **Full PR Check:** ~15 minutes total (parallel execution)

## Contributing

When adding new jobs:
1. Use latest action versions (v4+)
2. Add appropriate caching
3. Include in the `pr-checks-complete` dependency list
4. Test locally with Makefile targets first
5. Document any new secrets or requirements
