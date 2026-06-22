# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## [v0.13.0] - 2026-06-22

### Added
- 2026-06-22: Multi-select in the Source Manager TUI. Press `s` to toggle-select the item under the cursor and `Shift+↑/↓` to continuously range-select across items (header rows are skipped). With items selected, `l` installs or uninstalls them all in one confirmed batch (direction is install-all if any selected item is not installed, otherwise uninstall-all). `Esc` clears the selection. Selected items show a `●` marker and a count in the status line.

## [v0.12.0] - 2026-06-22

### Added
- 2026-06-22: The Source Manager and Tool Manager TUI headers now show the current version (`v{x.y.z}`) right-aligned in the title bar.

### Changed
- 2026-06-22: Adding a source in the Source Manager TUI now runs the clone/copy on a background thread, so the UI stays responsive for large repos (previously the terminal froze during the clone). The add no longer auto-installs every skill it finds — the new source appears in the list and you expand it to link the skills you want.
- 2026-06-22: `git clone` now uses `--depth 1` (shallow clone) to drastically reduce download size for large source repos. `git pull` updates still work.
- 2026-06-22: Bulk install in the Source Manager now reports skills/agents/commands skipped because their name is already in use (duplicate/vendored copies of the same skill). The status line and log explain the gap (e.g. `Installed 369 skill(s); skipped 143 duplicate-named`) instead of silently linking fewer than the total shown.
- 2026-06-22: The info popup (`i`) now summarizes duplicate-named entries: the repo info shows per-category duplicate counts within that source, and the category (skills/agents/commands root) info shows the total duplicate-named count across all sources — making it clear why the linked count can't reach the total.

## [v0.11.0] - 2026-06-03

### Added
- New `agm config` subcommand opens the config file (`~/.config/agm/config.toml`, or `--config` override) in `$EDITOR`, falling back to the platform default editor.

### Changed
- Removed the `config` entry from the TUI `agm` section. The config file is now edited via the dedicated `agm config` command rather than from inside the tool tree.
- Replaced the default `gemini` (Gemini CLI) tool with `agy` (Antigravity CLI), Google's successor terminal agent. It shares `~/.gemini` as its config dir, with settings at `antigravity-cli/settings.json`, MCP at `config/mcp_config.json`, and `GEMINI.md` as its global instructions file.

## [v0.10.0] - 2026-05-25

### Changed
- 2026-05-25: Renamed the `central` concept to `agm` throughout — the TUI tree section now reads `agm`, the config section is `[agm]`, and internal types are `AgmConfig`/`AgmField`. Existing configs using `[central]` still load via a serde alias and are re-saved as `[agm]`.

### Fixed
- 2026-05-25: Source skill/agent info popup can now scroll to the true bottom of word-wrapped content. Scroll bounds and the page indicator are now computed from the wrapped row count (via `Paragraph::line_count`) instead of the pre-wrap logical line count, so `End`/`PageDown` no longer stop short of the last section. The page indicator also snaps to the last page (e.g. `6/6`) once the bottom is reached, instead of reading `5/6`.

## [v0.9.1] - 2026-05-20

### Fixed
- `rename_relinks_installed_skill_only` test now checks the renamed segment via path components instead of a hardcoded `/new/` substring, fixing the Windows CI build where symlink targets use `\` separators.

## [v0.9.0] - 2026-05-20

### Breaking
- CLI restructured to use explicit subcommands. Removed: `agm tool -l/-u/-s`, `agm source -a/-u/-l/--add/--update/--list`. Replacements: `agm tool link|unlink|status`, `agm source add|update|list`.

### Added
- `agm source del <name|url>` — delete a source.
- `agm source rename <old> <new>` — rename a source folder and relink installed items.
- `agm source add -n,--name <name>` — override the cloned/copied directory name.
- TUI: `r` opens rename for the focused source row; `F5` refreshes the list.
- TUI info popups now display preload-char counts (`name` + `description` for skills; whole file for agents/commands) with rollups at source and category levels.

### Fixed
- `resolve_source_target` now canonicalizes shorthand `user/repo` URLs via `normalize_git_source` before matching, so `agm source del user/repo` correctly resolves repos cloned from `https://github.com/user/repo` (2026-05-20)
- TUI screen tearing when adding a source. All git stdout/stderr now flows through `LogBuffer`.
- TUI source rename input now supports the same keys as add input (Home/End, Left/Right, Delete); both share a new `TextInput` widget (2026-05-20)
- Clippy `lines_filter_map_ok` warnings in `clone_or_pull` stream readers; replaced `lines().flatten()` with `lines().map_while(Result::ok)`.
- `tui::tool::LinkContext` visibility raised to `pub` to match its use in `PopupState::Info`.
- Removed unused `editor` import from `src/main.rs`.

## [v0.8.2] - 2026-05-07

### Added
- Source repo entries in TUI now show installed/total count (e.g. `[7/7]`) instead of just `[7 skills]`
- Source TUI 'a' (add) now uses an inline input box instead of leaving the TUI; supports Home/End, Left/Right, Delete, Enter to confirm, Esc to cancel

### Fixed
- Update no longer re-installs skills that were explicitly uninstalled; a blocklist file (`.agm_uninstalled`) tracks intentional uninstalls and is respected during updates

## [v0.8.1] - 2026-04-28

### Fixed
- Windows tilde expansion, git shorthand URLs, source info popup 'e' key

## [v0.8.0] - 2026-04-17

### Added
- Info popup for CategoryHeader in source TUI (i key)
- Refactored tool TUI view with improved layout and rendering

### Changed
- Remove `source_repos` config field and related methods (`add_source_repo`, `remove_source_repo`)
- Simplify CLI by removing source repo management commands

## [v0.7.4] - 2026-04-04

### Fixed
- Prevent dangerous link operations when skills/agents/commands/prompt fields are empty or resolve to config_dir itself (e.g. `""`, `"."`)
- Centralize link path resolution via `ToolConfig::resolved_link_path()` with built-in safety checks
- Fix clippy warnings: collapse nested `if` blocks in `src/main.rs`

### Security
- Add `resolved_link_path()` safety guard to prevent config_dir collision when skills/agents/commands/prompt fields are empty or set to `"."`
- Centralize all link path resolution through `resolved_link_path()` for defense-in-depth against accidental data loss

## [v0.7.3] - 2026-04-03

### Fixed
- Fix Windows CI: use `platform::same_file` for idempotency check in install_command/install_agent
- Gate `test_prune_broken_commands` with `#[cfg(unix)]` (hard links can't become broken)

## [v0.7.2] - 2026-04-03

### Added
- Show link path for disabled features in tool TUI status rows (e.g., `commands disabled → ~/.claude/commands`)
- Central feature toggle — reorder, `i` key, popup, rendering, guards
- CLI `link_all`, `unlink_all`, and `status` skip disabled features
- Gray out disabled categories in source view with `i` key guard

### Changed
- Use `TOGGLEABLE_FEATURES` constant in `execute_toggle_feature`
- Review fixes — info popup disabled guard, `compute_tool_status`, refactor

## [v0.7.1] - 2026-04-02

### Fixed
- Fix Windows CI: use `platform::same_file` instead of `fs::read_link` in agent migration tests (hard links on Windows don't support `read_link`)

## [v0.7.0] - 2026-04-02

### Breaking Changes
- **CLI consolidation:** Removed subcommands `link`, `unlink`, `status`, `config`, `prompt`, `auth`, `mcp`. All tool management is now under `agm tool`:
  - `agm tool` — Interactive TUI for managing tools, links, and configuration
  - `agm tool --link` — Link all tools (replaces `agm link`)
  - `agm tool --unlink` — Unlink all tools (replaces `agm unlink`)
  - `agm tool --status` — Show status table (replaces `agm status`)
  - Editing config/prompt/auth/mcp files is done via the TUI (`e` key)

### Added
- **Tool TUI** (`agm tool`): Interactive terminal UI for managing tools
  - View and toggle link status (prompt, skills, agents) per tool
  - Edit files with `e` key, file picker popup for multi-file fields
  - Inline path editing for central config paths
  - Expand/collapse sections with space/enter, 0/9 for all
  - Log popup (`l` key) showing operation history
- **Source TUI improvements:**
  - Non-blocking background updates (TUI stays responsive during `git pull`)
  - Scrollable info popup (`i` key) showing skill/agent details and SKILL.md content
  - Log popup (`l` key) with timestamped operation history
- Shared TUI infrastructure: `ScrollablePopup`, `LogBuffer`, `BackgroundTask` modules
- Integration tests for new CLI structure

### Changed
- Moved `manage.rs` → `src/tui/source.rs` as part of TUI module reorganization
- Extracted `migrate_tool_dir()` and `copy_dir_all()` to `skills.rs` for reuse
- Added quiet linker variants (`create_link_quiet`, `remove_link_quiet`) for TUI use

## [v0.6.0] - 2026-04-01

### Fixed
- Fix Windows CI test failure: gate `test_prune_broken_agents` with `#[cfg(unix)]`
  since hard links (used on Windows) cannot become "broken" like symlinks

### Added
- **Agents management**: Support for agent `.md` files alongside directory-based skills
  - Central agents store at `~/.local/share/agm/agents/`
  - `agents_dir` field in each tool config for per-tool agent directories
  - Agent discovery from `agents/` folders in source repos
  - Agent install/uninstall/prune operations
- **3 new default tools** (7 total): Codex CLI, Pi, Crush
- **Interactive TUI overhaul** with 3-level hierarchy (Category → Source → Item)
  - Collapse/expand with space/enter, `0` (collapse all), `9` (expand all)
  - Fuzzy search with `/` key
  - Quick keys: `a` (add), `u` (update), `d` (delete)
  - Auto-update on TUI launch
  - Dual-panel view: Skills section + Agents section

### Changed
- **BREAKING**: `agm skills` command renamed to `agm source`
  - Subcommands replaced with flags: `--add/-a`, `--update/-u`, `--list/-l`
  - No arguments opens interactive TUI directly
- **BREAKING**: Config schema changes
  - `skill_repos` renamed to `source_repos` in `[central]`
  - `agents_source` added to `[central]` (default: `~/.local/share/agm/agents`)
  - `agents_dir` added to `[tools.*]` (default: `agents`)
- `agm link`/`agm unlink` now handle agents in addition to prompts and skills

### Removed
- **BREAKING**: Removed `files_base` and `files` from config (central and per-tool)
- Removed `files.rs` module and all file-linking logic
- Removed per-source management from `agm source` (use TUI instead)

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
