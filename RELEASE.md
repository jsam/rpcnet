# Release Guide

This document describes how to create a new release of rpcnet and publish it to crates.io.

## Prerequisites

- You have write access to the repository
- Repository secrets are configured:
  - `CARGO_REGISTRY_TOKEN` - Token from crates.io for publishing
  - `CODECOV_TOKEN` - Token for uploading coverage reports

## Release Methods

There are three ways to trigger a release:

### Method 1: Tag Push (Recommended)

This is the simplest method. Just create and push a version tag:

```bash
# 1. Update version in Cargo.toml
vim Cargo.toml  # Change version to e.g., 0.2.1

# 2. Commit the version change
git add Cargo.toml
git commit -m "chore: bump version to 0.2.1"

# 3. Create and push the tag
git tag v0.2.1
git push origin v0.2.1

# The release workflow will automatically:
# - Run all pre-publish checks
# - Verify version matches tag
# - Publish to crates.io
# - Build release binaries
# - Verify the published package
```

### Method 2: GitHub Release

Create a release through the GitHub UI:

```bash
# 1. Update version in Cargo.toml
vim Cargo.toml

# 2. Commit and push
git add Cargo.toml
git commit -m "chore: bump version to 0.2.1"
git push

# 3. Go to GitHub > Releases > Create a new release
# - Tag: v0.2.1
# - Title: Release 0.2.1
# - Description: Add changelog
# - Click "Publish release"

# The workflow will run automatically
```

### Method 3: Manual Workflow Dispatch

Trigger the workflow manually from GitHub Actions:

```bash
# 1. Go to Actions > Release > Run workflow
# 2. Select branch
# 3. Enter version (e.g., 0.2.1)
# 4. Choose dry run or actual publish
```

## Pre-Release Checklist

Before creating a release, ensure:

- [ ] All tests pass locally: `make test`
- [ ] Code is formatted: `make format`
- [ ] No clippy warnings: `make lint`
- [ ] Coverage meets threshold: `make coverage-check`
- [ ] Examples compile: `make examples`
- [ ] Documentation builds: `make doc`
- [ ] CHANGELOG.md is updated
- [ ] Version in Cargo.toml is correct
- [ ] All PRs are merged to main

Quick check everything:
```bash
make publish-check
```

## Release Workflow Steps

When you push a tag, the workflow automatically:

### 1. Validation (5-10 minutes)
- ✅ Formats code
- ✅ Runs Clippy
- ✅ Runs all tests on multiple platforms
- ✅ Checks coverage threshold
- ✅ Verifies version matches tag
- ✅ Runs security audit
- ✅ Builds documentation

### 2. Publishing (2-3 minutes)
- ✅ Publishes to crates.io
- ✅ Creates job summary with install instructions

### 3. Build Artifacts (10-15 minutes)
- ✅ Builds `rpcnet-gen` binary for:
  - Linux x86_64
  - macOS x86_64
  - macOS ARM64 (Apple Silicon)
- ✅ Uploads artifacts to workflow run

### 4. Verification (2-3 minutes)
- ✅ Waits for crates.io to index
- ✅ Verifies package is available
- ✅ Tests installation

### 5. Announcement
- ✅ Creates summary with all links
- ✅ Marks workflow as successful

Total time: ~20-30 minutes

## After Release

Once the workflow completes:

1. **Verify on crates.io**
   - Visit https://crates.io/crates/rpcnet
   - Confirm new version is listed

2. **Check documentation**
   - Visit https://docs.rs/rpcnet
   - Confirm docs are building (may take 5-10 minutes)

3. **Download binaries** (optional)
   - Go to Actions > Release workflow run
   - Download artifacts from the artifacts section

4. **Update dependent projects** (if any)
   ```bash
   cargo update -p rpcnet
   ```

## Dry Run Testing

To test the release process without publishing:

```bash
# Method 1: Using make
make publish-dry-run

# Method 2: Using GitHub Actions
# Go to Actions > Release > Run workflow
# - Check "Perform dry run only"
# - Click Run workflow
```

This will:
- Run all validation checks
- Show what would be published
- NOT actually publish to crates.io

## Troubleshooting

### Version Mismatch Error

```
❌ Error: Version mismatch!
Cargo.toml version (0.2.0) does not match git tag (0.2.1)
```

**Fix:**
```bash
# Update Cargo.toml to match tag
vim Cargo.toml

# Commit and re-tag
git add Cargo.toml
git commit -m "fix: update version to 0.2.1"
git tag -f v0.2.1  # Force update tag
git push -f origin v0.2.1
```

### Tests Failing

If tests fail during release:

```bash
# Run checks locally
make publish-check

# Fix issues and try again
git add .
git commit -m "fix: resolve test failures"
git push
git tag -f v0.2.1
git push -f origin v0.2.1
```

### Publishing Failed

If publishing to crates.io fails:

1. Check `CARGO_REGISTRY_TOKEN` is valid
2. Verify you have permission to publish
3. Check crates.io status: https://status.crates.io
4. Re-run the workflow from Actions UI

### Binary Builds Failed

Binary builds may fail on certain platforms. This is non-blocking:
- The package will still be published to crates.io
- Users can build from source
- File an issue for investigation

## Yanking a Release

If you need to yank a broken release:

```bash
# Yank from crates.io
cargo yank --vers 0.2.1

# Or undo the yank
cargo yank --vers 0.2.1 --undo
```

⚠️ **Note:** Yanking doesn't delete the release, it just prevents new projects from using it.

## Release Checklist Template

Copy this for each release:

```markdown
## Release 0.x.x Checklist

### Pre-Release
- [ ] Update CHANGELOG.md
- [ ] Update version in Cargo.toml
- [ ] Run `make publish-check` locally
- [ ] All tests passing on CI
- [ ] Documentation reviewed

### Release
- [ ] Create and push tag: `git tag v0.x.x && git push origin v0.x.x`
- [ ] Verify workflow started on GitHub Actions
- [ ] Monitor workflow progress (~20-30 min)

### Post-Release
- [ ] Verify on crates.io: https://crates.io/crates/rpcnet
- [ ] Check docs.rs: https://docs.rs/rpcnet
- [ ] Download and test binaries
- [ ] Announce on social media / Discord / etc. (optional)
- [ ] Update dependent projects

### Rollback (if needed)
- [ ] `cargo yank --vers 0.x.x`
- [ ] Fix issues
- [ ] Release patch version
```

## Version Numbers

Follow semantic versioning (https://semver.org/):

- **Patch** (0.2.1): Bug fixes, no breaking changes
- **Minor** (0.3.0): New features, no breaking changes  
- **Major** (1.0.0): Breaking changes

For pre-1.0 releases, minor versions may contain breaking changes.

## Getting Help

If you encounter issues:

1. Check workflow logs in GitHub Actions
2. Review this guide
3. Check `.github/workflows/release.yml` for details
4. Open an issue if you find a bug in the release process

## Security

- Never commit `CARGO_REGISTRY_TOKEN` to the repository
- Always use GitHub Secrets for sensitive tokens
- Review the diff before pushing tags
- Be cautious with force-pushing tags

---

# Cheat sheet

  # 1. Prepare release (runs on local machine)
  make release-prepare VERSION=0.2.0

  # 2. Create PR (standard pr-checks.yml validates)
  git push -u origin release/0.2.0

  # 3. Merge PR to main

  # 4. Tag release (triggers release.yml workflow)
  git tag -a 0.2.0 -m "Release 0.2.0"
  git push origin 0.2.0  # 🚀 Automatically publishes to crates.io