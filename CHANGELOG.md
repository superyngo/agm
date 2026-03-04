# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [v0.2.1] - 2026-03-04

### Changed
- Improve config and status handling internals

## [v0.2.0] - 2026-02-25

### Added
- Add centralized file path management
- Add file status checking (linked, broken, wrong, missing, etc.)
- Add link/unlink file operations with proper handling
- Add comprehensive test coverage for file operations

### Changed
- Refactor paths, skills, and status modules

## [v0.1.2] - 2026-02-25

### Fixed
- Fix opencode default auth path to `~/.local/share/opencode/auth.json`
- Fix `agm link`: prompt with wrong symlink target now prompts user to re-link (same as skills behavior)

## [0.1.1] - 2026-02-14

### Added
- Support for lowercase `-v` flag to display version information
- Interactive tool selection menu for `agm edit auth` and `agm edit mcp` commands when tool not specified
- Full help text display when required command parameters are missing (instead of brief error)
- GitHub Actions release workflow for automated releases

### Fixed
- Fixed panic when running `agm` with no arguments - now shows help text instead

### Changed
- **BREAKING**: Unified edit command syntax from `agm edit <tool> <file_type>` to `agm edit <file_type> [tool]`
  - Old: `agm edit claude config`
  - New: `agm edit config claude`
- Version flag now uses lowercase `-v` instead of uppercase `-V`

### Migration Guide

**Edit Command Syntax Change:**
- Before (v0.1.0): `agm edit <tool> <file_type>`
- After (v0.1.1): `agm edit <file_type> [tool]`

Examples:
- `agm edit claude config` → `agm edit config claude`
- `agm edit gemini auth` → `agm edit auth gemini`
- `agm edit prompt` → **unchanged** (opens MASTER.md)
- `agm edit config` → **unchanged** (opens agm config.toml)

## [0.1.0] - 2026-02-14

### Added
- Initial AGM (AI Agent Manager) v0.1.0 implementation
- Core commands: init, status, list, check, link, unlink, skills, edit
- Registry-driven tool configuration with 4 default tools (claude, gemini, copilot, opencode)
- Symlink management for prompts and skills directories
- Skills management: local path and git URL support
- Auto-update for skill repositories
- Editor integration with $EDITOR support
- Comprehensive test suite with 20 unit tests
- Full documentation: README.md and design doc
