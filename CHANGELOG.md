# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.3.1] - 2026-03-18

### Removed
- Remove `agm list` command - functionality now covered by `agm status`
- Remove `agm check` command - functionality now covered by `agm status`

## [v0.3.0] - 2026-03-06

### Added
- Add `agm skills list` subcommand
- Add interactive action picker for `agm skills` without argument (list/add/remove/update)

### Changed
- Promote `edit` subcommands to top-level commands (`prompt`, `config`, `auth`, `mcp`)
- Add global `--config <path>` override option
- Replace multi-file open-all with interactive `dialoguer` picker
- `--config` now propagates to `init` command
- `skills update` re-syncs central symlinks after git pull
- `link`/`unlink` replace `--all` flag with positional `target` (all/central/tool)
- Rename `agm` target to `central` in `prompt`/`config` commands
- All commands with optional target now show interactive `dialoguer` picker instead of exiting

## [v0.5.0] - 2026-03-21

### Added
- Interactive TUI skill manager using ratatui and crossterm
- Status display shows skill install count from scan_all_sources
- Delete source function for managing skill sources
- Add local copy function to copy skills from source directory
- Clone or pull function split from install operation
- Scan all sources with source grouping and install status tracking
- Install skill and uninstall skill functions
- SkillInfo, SourceGroup, and SkillInstallStatus types
- Remove skill repo method to Config
- Ratatui and crossterm dependencies for TUI support

### Changed
- Refactor: remove old add_local, add_from_url, remove_skill, list_skills functions
- Update CLI to add multi-select and manage subcommand
- Deprecate remove command (use manage instead)
- Update_all now uses source_dir scanning and install_skill

### Fixed
- Normalize git URLs for comparison and track bulk toggle errors
- Resolve clippy warnings (boolean simplification, loop indexing, print literal)
- Fix Cargo.toml version conflicts
- Remove empty prompt files before linking
- Show file path in blocked status display
- Use platform-native path separators in contract_tilde

### Docs
- Add implementation plan for prompt blocked and display fixes
- Add design spec for prompt blocked handling and display fixes

## [v0.4.0] - 2026-03-20

### Added
- Windows platform support with NTFS junctions for directories and hardlinks for files
- Platform abstraction layer for cross-platform link operations
- Windows CI/CD targets in GitHub Actions
- Link capability detection for Windows systems

### Changed
- Code formatting improvements via cargo fmt
- Improve link error message formatting

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
