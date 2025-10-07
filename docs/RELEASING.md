# Release Process

This document describes the release process for RpcNet using conventional commits and automated tooling.

## Overview

RpcNet uses a semi-automated release process that:
- ✅ Generates changelogs automatically from conventional commit messages
- ✅ Bumps versions consistently across all files
- ✅ Creates release branches following a standard naming convention
- ✅ Validates releases through automated PR checks
- ✅ Ensures reproducible and documented releases

The process is designed to be simple, requiring just a single command to prepare a release.

## Prerequisites

### Required Tools

The release preparation script will automatically install missing tools, but you can install them manually:

```bash
# Install git-cliff for changelog generation (required)
cargo install git-cliff

# Install cargo-release for version management (optional, not currently used)
cargo install cargo-release
```

### Repository Requirements

- Clean working directory (uncommitted changes can be stashed)
- Access to push to the repository
- Familiarity with conventional commit format (see below)

### Understanding the Release Files

The following files control the release process:

| File | Purpose |
|------|---------|
| `cliff.toml` | Configures git-cliff for changelog generation |
| `.release.toml` | Configures cargo-release (optional) |
| `scripts/prepare-release.sh` | Main automation script for releases |
| `.github/workflows/release-pr.yml` | GitHub Actions workflow for PR validation |
| `docs/RELEASING.md` | This document - release process guide |

## Quick Start

For the impatient, here's the TL;DR:

```bash
# Prepare release
make release-prepare VERSION=0.2.0

# Push and create PR
git push -u origin release/0.2.0

# After PR merge
git checkout main && git pull
git tag 0.2.0 && git push origin 0.2.0
```

## Detailed Release Process

### 1. Prepare Release

Run the automated release preparation script using the Makefile:

```bash
# Using Makefile (recommended)
make release-prepare VERSION=<new-version>

# Or directly using the script
./scripts/prepare-release.sh <new-version>
```

**Examples:**

```bash
# Patch release (bug fixes only)
make release-prepare VERSION=0.1.1

# Minor release (new features, backwards compatible)
make release-prepare VERSION=0.2.0

# Major release (breaking changes)
make release-prepare VERSION=1.0.0

# Pre-release versions
make release-prepare VERSION=0.2.0-beta.1
make release-prepare VERSION=1.0.0-rc.1
```

**What the script does:**

1. ✅ Validates version format (semver)
2. ✅ Creates release branch: `release/<version>`
3. ✅ Generates/updates `CHANGELOG.md` from conventional commits
4. ✅ Updates version in `Cargo.toml`
5. ✅ Updates version references in `README.md`
6. ✅ Updates `Cargo.lock`
7. ✅ Creates a single commit with all changes
8. ✅ Provides next steps instructions

**Output Example:**

```
Current version: 0.1.0
Preparing release 0.2.0
Creating release branch: release/0.2.0
Generating changelog...
Updating version in Cargo.toml...
Updating version in README.md...
Updating Cargo.lock...
Committing changes...

✅ Release preparation complete!

Next steps:
  1. Review the changes: git show
  2. Push the branch: git push -u origin release/0.2.0
  3. Create a PR on GitHub
  4. After PR is merged, tag the release: git tag 0.2.0 && git push origin 0.2.0
```

### 2. Review Changes

Before pushing, review what was changed:

```bash
# Review the commit
git show

# Check the diff
git diff HEAD~1

# Review the generated changelog
cat CHANGELOG.md

# Check updated files
git status
```

### 3. Create Release PR

Push the release branch and create a PR:

```bash
# Push the release branch
git push -u origin release/<version>

# Example
git push -u origin release/0.2.0
```

Then create a Pull Request on GitHub:
1. Go to https://github.com/jsam/rpcnet/compare
2. Select `main` as the base branch
3. Select your `release/<version>` branch as the compare branch
4. Create the pull request

**Automated PR Checks:**

The `.github/workflows/release-pr.yml` workflow will automatically:

- ✅ Validate version consistency across files
- ✅ Check that CHANGELOG.md exists and contains the new version
- ✅ Verify conventional commit format
- ✅ Run full test suite
- ✅ Post helpful comment with:
  - Release checklist
  - Post-merge instructions
  - Manual release commands

**Example PR Comment:**

```markdown
## 🚀 Release PR for 0.2.0

This PR prepares the release for version **0.2.0**.

### Checklist
- ✅ Version updated in `Cargo.toml`
- ✅ `CHANGELOG.md` generated
- ✅ All tests passing

### After Merge
Once this PR is merged to main:
1. Create and push the git tag: `git tag 0.2.0 && git push origin 0.2.0`
2. The release workflow will automatically publish to crates.io
3. GitHub release will be created automatically
```

### 4. Review and Merge

**Review Checklist:**

- [ ] CHANGELOG.md accurately reflects changes
- [ ] Version number is correct in all files
- [ ] All CI checks pass (format, lint, tests, coverage)
- [ ] No unexpected files were modified
- [ ] Commit message follows format

**Approval and Merge:**

Once approved by maintainers, merge the PR to `main` using:
- "Squash and merge" (recommended) - creates clean history
- "Merge commit" - preserves all commits

### 5. Create Git Tag

**After the PR is merged** to main, create and push the version tag:

```bash
# 1. Switch to main and pull latest
git checkout main
git pull origin main

# 2. Create annotated tag with message
git tag -a <version> -m "Release <version>"

# 3. Push tag to trigger release workflow
git push origin <version>
```

**Example:**
```bash
git checkout main
git pull origin main
git tag -a 0.2.0 -m "Release 0.2.0"
git push origin 0.2.0
```

**What happens next:**

Pushing the tag will trigger:
- 📦 Package creation
- 🧪 Full test suite
- 📤 Publication to crates.io (if workflow is configured)
- 🎉 GitHub Release creation with changelog

### 6. Publish to crates.io

**Automatic Publication (if configured):**

If the release workflow is set up with `CARGO_REGISTRY_TOKEN`, publication happens automatically when you push the tag.

**Manual Publication:**

If automatic publication isn't configured, publish manually:

```bash
# 1. Ensure you're on the tagged commit
git checkout 0.2.0

# 2. Run pre-publication checks
make publish-check

# 3. Dry run to verify package contents
make publish-dry-run

# 4. Publish to crates.io
make publish
# or
cargo publish

# 5. Verify on crates.io
open https://crates.io/crates/rpcnet
```

### 7. Verify Release

After publishing, verify everything worked:

**Checklist:**

- [ ] Check crates.io: https://crates.io/crates/rpcnet
- [ ] Verify docs.rs built: https://docs.rs/rpcnet
- [ ] Check GitHub release: https://github.com/jsam/rpcnet/releases
- [ ] Test installation: `cargo install rpcnet --version <version>`
- [ ] Verify badge updates in README

**Test Installation:**

```bash
# Create test project
cargo new test-rpcnet
cd test-rpcnet

# Add dependency
cargo add rpcnet@0.2.0

# Verify it works
cargo build
```

## Conventional Commit Format

We use [Conventional Commits](https://www.conventionalcommits.org/) for automatic changelog generation:

### Commit Types

- `feat:` - New feature (shows in "Features" section)
- `fix:` - Bug fix (shows in "Bug Fixes" section)
- `docs:` - Documentation changes
- `perf:` - Performance improvements
- `refactor:` - Code refactoring
- `style:` - Code style changes (formatting, etc.)
- `test:` - Adding or updating tests
- `chore:` - Maintenance tasks
- `ci:` - CI/CD changes

### Examples

```bash
# Feature
git commit -m "feat: add connection pooling support"
git commit -m "feat(cluster): implement health checking"

# Bug fix
git commit -m "fix: resolve race condition in client reconnect"
git commit -m "fix(server): handle graceful shutdown properly"

# Breaking change
git commit -m "feat!: redesign cluster API for better ergonomics

BREAKING CHANGE: ClusterClient now requires explicit configuration"

# With scope and body
git commit -m "perf(codec): optimize serialization path

Reduces allocations by 40% in hot path by reusing buffers."
```

### Commit Scopes (optional)

Common scopes:
- `cluster` - Cluster management
- `client` - RPC client
- `server` - RPC server
- `codec` - Serialization/deserialization
- `tls` - TLS/security
- `docs` - Documentation
- `tests` - Testing infrastructure

## Versioning Strategy

We follow [Semantic Versioning](https://semver.org/):

- **MAJOR** (1.0.0): Breaking changes
- **MINOR** (0.2.0): New features, backwards compatible
- **PATCH** (0.1.1): Bug fixes, backwards compatible

### Pre-releases

Use pre-release versions for beta/alpha releases:

- `0.2.0-beta.1` - Beta release
- `0.2.0-alpha.1` - Alpha release
- `0.2.0-rc.1` - Release candidate

## Advanced Usage

### Manual Changelog Generation

Generate changelog manually without creating a release:

```bash
# Using Makefile
make changelog                    # Generate for all changes
make changelog VERSION=0.2.0      # Generate for specific version

# Using git-cliff directly
git-cliff -o CHANGELOG.md                  # All changes
git-cliff --tag 0.2.0 -o CHANGELOG.md    # Specific version
git-cliff --tag 0.2.0                    # Preview only (no file)
git-cliff --unreleased                    # Only unreleased changes
```

### Preview Changelog Before Release

Preview what will be in the changelog:

```bash
# Preview unreleased changes
git-cliff --unreleased

# Preview changes for next version
git-cliff --tag 0.2.0 --unreleased
```

### Updating Existing Releases

If you need to regenerate the entire changelog:

```bash
# Regenerate complete changelog from all tags
git-cliff --tag 0.2.0 -o CHANGELOG.md

# Or manually edit CHANGELOG.md and commit
vim CHANGELOG.md
git add CHANGELOG.md
git commit -m "docs: update changelog"
```

## Common Workflows

### Hotfix Release (Patch Version)

For urgent bug fixes:

```bash
# 1. Prepare hotfix release
make release-prepare VERSION=0.1.1

# 2. Push and create PR
git push -u origin release/0.1.1

# 3. Fast-track review and merge

# 4. Tag immediately
git checkout main && git pull
git tag -a 0.1.1 -m "Hotfix: critical bug fixes"
git push origin 0.1.1
```

### Beta/RC Releases

For pre-releases before a major version:

```bash
# Beta release
make release-prepare VERSION=0.2.0-beta.1

# Release candidate
make release-prepare VERSION=1.0.0-rc.1

# Follow standard process, then:
cargo publish  # Pre-releases won't affect stable users
```

### Yanking a Release

If you need to yank a published release:

```bash
# Yank from crates.io (prevents new usage)
cargo yank --vers 0.2.0

# Undo yank if needed
cargo yank --vers 0.2.0 --undo

# Document in changelog
echo "**Note:** Version 0.2.0 was yanked due to [reason]" >> CHANGELOG.md
```

## Troubleshooting

### Script Fails: "Not on main branch"

```bash
# If you're confident, you can continue from another branch
# The script will prompt you to confirm

# Or switch to main first
git checkout main
git pull
make release-prepare VERSION=0.2.0
```

### Version Mismatch in CI

If the release PR checks fail with version mismatch:

```bash
# Update version in Cargo.toml manually
vim Cargo.toml

# Regenerate changelog
git-cliff --tag 0.2.0 -o CHANGELOG.md

# Commit and push
git add Cargo.toml CHANGELOG.md
git commit --amend --no-edit
git push -f
```

### Missing Conventional Commits

If commits don't follow conventional format, they won't appear in changelog:

```bash
# Check recent commits
git log --oneline -20

# Check what will be in changelog
git-cliff --unreleased

# Rewrite commit messages if needed (before pushing)
git rebase -i HEAD~5

# Update each commit to use conventional format:
# feat: add new feature
# fix: resolve bug
# docs: update documentation
```

### Uncommitted Changes

```bash
# Script will prompt to stash
# Or commit/stash manually first
git stash
make release-prepare VERSION=0.2.0
git stash pop
```

### Release Branch Already Exists

```bash
# Delete old branch locally and remotely
git branch -D release/0.2.0
git push origin --delete release/0.2.0

# Then run release prepare again
make release-prepare VERSION=0.2.0
```

### CI Checks Failing

```bash
# Run checks locally before pushing
make pre-commit          # Quick checks
make publish-check       # Full pre-publication checks

# Fix issues and amend commit
git add .
git commit --amend --no-edit
git push -f origin release/0.2.0
```

### Changelog not generated

```bash
# Install/reinstall git-cliff
cargo install git-cliff --force

# Check configuration
cat cliff.toml

# Generate manually
git-cliff --tag 0.2.0 -o CHANGELOG.md
```

## Configuration Files

- `cliff.toml` - git-cliff configuration for changelog generation
- `.release.toml` - cargo-release configuration (optional)
- `scripts/prepare-release.sh` - Automated release preparation script
- `.github/workflows/release-pr.yml` - Release PR validation workflow
- `.github/workflows/release.yml` - Automated release publishing workflow

## Best Practices

1. **Always use conventional commits** - This ensures good changelogs
2. **Write descriptive commit bodies** - Helps reviewers understand changes
3. **Test before releasing** - Run `cargo test --all-features`
4. **Review generated changelog** - Edit if needed before merging PR
5. **Document breaking changes** - Use `BREAKING CHANGE:` in commit body
6. **Keep main stable** - All releases go through PR review
7. **Tag immediately after merge** - Don't delay tagging the release

## Complete Example: Version 0.2.0 Release

Here's a complete walkthrough of releasing version 0.2.0:

```bash
# ============================================
# STEP 1: PREPARE RELEASE
# ============================================

# Start from a clean main branch
git checkout main
git pull origin main

# Prepare the release
make release-prepare VERSION=0.2.0

# Output:
# Current version: 0.1.0
# Preparing release 0.2.0
# Creating release branch: release/0.2.0
# Generating changelog...
# Updating version in Cargo.toml...
# ✅ Release preparation complete!

# Review changes
git show
cat CHANGELOG.md

# ============================================
# STEP 2: PUSH AND CREATE PR
# ============================================

# Push release branch
git push -u origin release/0.2.0

# Create PR on GitHub
# https://github.com/jsam/rpcnet/compare/main...release/0.2.0

# Wait for CI checks to pass
# Wait for review and approval

# ============================================
# STEP 3: MERGE PR
# ============================================

# Merge PR on GitHub (squash and merge)

# ============================================
# STEP 4: TAG RELEASE
# ============================================

# Pull merged changes
git checkout main
git pull origin main

# Create and push tag
git tag -a 0.2.0 -m "Release 0.2.0"
git push origin 0.2.0

# ============================================
# STEP 5: PUBLISH
# ============================================

# If automatic publishing isn't set up:
make publish

# Or wait for GitHub Actions to publish

# ============================================
# STEP 6: VERIFY
# ============================================

# Check crates.io
open https://crates.io/crates/rpcnet

# Check docs.rs
open https://docs.rs/rpcnet

# Check GitHub release
open https://github.com/jsam/rpcnet/releases

# Test installation
cargo install rpcnet --version 0.2.0

# ✅ Release complete!
```

## References

- [Conventional Commits](https://www.conventionalcommits.org/)
- [Semantic Versioning](https://semver.org/)
- [git-cliff Documentation](https://git-cliff.org/)
- [cargo-release Documentation](https://github.com/crate-ci/cargo-release)
