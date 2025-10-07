#!/usr/bin/env bash
# Prepare a release PR with version bump and changelog generation
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

echo -e "${GREEN}Current version: ${CURRENT_VERSION}${NC}"

# Check if version argument is provided
if [ $# -eq 0 ]; then
    echo -e "${YELLOW}Usage: $0 <new-version>${NC}"
    echo "Example: $0 0.2.0"
    echo "         $0 0.1.1"
    echo "         $0 1.0.0"
    exit 1
fi

NEW_VERSION=$1

# Validate version format (semver)
if ! [[ $NEW_VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo -e "${RED}Error: Invalid version format. Must be semver (e.g., 1.2.3 or 1.2.3-beta.1)${NC}"
    exit 1
fi

echo -e "${GREEN}Preparing release ${NEW_VERSION}${NC}"

# Check if git-cliff is installed
if ! command -v git-cliff &> /dev/null; then
    echo -e "${YELLOW}git-cliff not found. Installing...${NC}"
    cargo install git-cliff
fi

# Check if cargo-release is installed
if ! command -v cargo-release &> /dev/null; then
    echo -e "${YELLOW}cargo-release not found. Installing...${NC}"
    cargo install cargo-release
fi

# Ensure we're on main branch
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT_BRANCH" != "main" ]; then
    echo -e "${YELLOW}Warning: You're not on the main branch (currently on: ${CURRENT_BRANCH})${NC}"
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check for uncommitted changes
if [ -n "$(git status --porcelain)" ]; then
    echo -e "${YELLOW}Warning: You have uncommitted changes${NC}"
    git status --short
    read -p "Stash changes and continue? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git stash
    else
        exit 1
    fi
fi

# Create release branch
RELEASE_BRANCH="release/${NEW_VERSION}"
echo -e "${GREEN}Creating release branch: ${RELEASE_BRANCH}${NC}"
git checkout -b "$RELEASE_BRANCH"

# Generate changelog
echo -e "${GREEN}Generating changelog...${NC}"
git-cliff --tag "${NEW_VERSION}" -o CHANGELOG.md

# Update version in Cargo.toml
echo -e "${GREEN}Updating version in Cargo.toml...${NC}"
sed -i.bak "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" Cargo.toml
rm Cargo.toml.bak

# Update version in README.md
echo -e "${GREEN}Updating version in README.md...${NC}"
sed -i.bak "s/rpcnet = \"[^\"]*\"/rpcnet = \"${NEW_VERSION}\"/" README.md
rm README.md.bak

# Update lock file
echo -e "${GREEN}Updating Cargo.lock...${NC}"
cargo check --quiet

# Show changes
echo -e "${GREEN}Changes to be committed:${NC}"
git diff --stat

# Commit changes
echo -e "${GREEN}Committing changes...${NC}"
git add Cargo.toml Cargo.lock CHANGELOG.md README.md
git commit -m "chore(release): prepare for ${NEW_VERSION}

- Update version to ${NEW_VERSION}
- Generate changelog for ${NEW_VERSION}
- Update documentation version references"

echo ""
echo -e "${GREEN}✅ Release preparation complete!${NC}"
echo ""
echo -e "Next steps:"
echo -e "  1. Review the changes: ${YELLOW}git show${NC}"
echo -e "  2. Push the branch: ${YELLOW}git push -u origin ${RELEASE_BRANCH}${NC}"
echo -e "  3. Create a PR on GitHub"
echo -e "  4. After PR is merged, tag the release: ${YELLOW}git tag ${NEW_VERSION} && git push origin ${NEW_VERSION}${NC}"
echo ""
