# Release Guide

This document explains how to create a new release for AGM.

## Prerequisites

1. Ensure all changes are committed and tests pass
2. Update version in `Cargo.toml`
3. Update `CHANGELOG.md` with release notes

## Creating a Release

### Method 1: Push Tag (Recommended)

1. Create and push an annotated tag:

```bash
# Create annotated tag with message
git tag -a v0.1.0 -m "Release v0.1.0

Features:
- Initial release
- Centralized configuration management
- Skills management
- Symlink handling"

# Push the tag to GitHub
git push origin v0.1.0
```

2. The GitHub Actions workflow will automatically:
   - Build binaries for all supported platforms
   - Create checksums (SHA256SUMS)
   - Create a GitHub release with all artifacts
   - Generate changelog from commits

### Method 2: Manual Workflow Trigger

You can also manually trigger the workflow from the GitHub Actions tab:

1. Go to Actions → Release Build
2. Click "Run workflow"
3. Enter the tag name (e.g., `v0.1.0`)
4. Optionally mark as draft release
5. Click "Run workflow"

## Supported Platforms

The release workflow builds for:

### Linux
- x86_64 (GNU and musl)
- i686 (GNU and musl)
- aarch64 (GNU and musl)
- armv7 (GNU and musl)

### macOS
- x86_64 (Intel)
- aarch64 (Apple Silicon)

## Artifact Naming

Release artifacts follow this pattern:
- Linux: `agm-linux-{arch}[-musl].tar.gz`
- macOS: `agm-macos-{arch}.tar.gz`
- Checksums: `SHA256SUMS`

## Post-Release

After a release is created:

1. Verify all artifacts are uploaded
2. Test download and installation on different platforms
3. Update documentation if needed
4. Announce the release

## Version Numbering

AGM follows [Semantic Versioning](https://semver.org/):
- MAJOR version for incompatible API changes
- MINOR version for new functionality
- PATCH version for bug fixes

Example: `v1.2.3`
- `1` = Major version
- `2` = Minor version
- `3` = Patch version
