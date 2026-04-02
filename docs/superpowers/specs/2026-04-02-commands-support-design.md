# Commands Support — Design Spec

## Problem

AGM currently manages two types of markdown-based resources from source repos: **skills** (directories with `SKILL.md`) and **agents** (`.md` files in `agents/` folders). Users also have **custom slash commands** — markdown prompt templates stored in `commands/` folders — that need the same central management: discovery from source repos, install/uninstall via symlinks, migration from existing tool directories, and TUI browsing.

## Approach

**Full parallel implementation** (方案 A): replicate the agents pattern for commands across all layers. Each function is short (10–30 lines), so duplication cost is low and consistency with the existing codebase is maximised.

---

## 1. Config Layer

### `CentralConfig` (`src/config.rs`)

Add field:

```rust
#[serde(default = "CentralConfig::default_commands_source")]
pub commands_source: String,
```

Default: `"~/.local/share/agm/commands"`

Add default function:

```rust
fn default_commands_source() -> String {
    "~/.local/share/agm/commands".into()
}
```

### `ToolConfig` (`src/config.rs`)

Add field:

```rust
#[serde(default)]
pub commands_dir: String,
```

### `default_config()` (`src/config.rs`)

- Every tool gets `commands_dir: "commands".into()`
- `CentralConfig` gets `commands_source: "~/.local/share/agm/commands".into()`

### `init.rs`

- Add `&config.central.commands_source` to the `dirs_to_create` array so `agm init` creates the central commands directory.

---

## 2. Data Layer (`src/skills.rs`)

### New struct

```rust
#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub name: String,
    pub source_path: PathBuf,
    pub install_status: SkillInstallStatus, // reuse existing enum
}
```

### `SourceGroup` change

Add field:

```rust
pub commands: Vec<CommandInfo>,
```

### New functions

All mirror the agents equivalents but operate on `commands/` subdirectory:

| Function | Signature | Behaviour |
|----------|-----------|-----------|
| `scan_commands` | `(path: &Path) → Vec<(String, PathBuf)>` | Scan `{path}/commands/` for `.md` files, return `(stem, path)` sorted |
| `install_command` | `(name, source_path, commands_dir) → Result<()>` | Create file symlink `{commands_dir}/{name}.md → source_path` |
| `uninstall_command` | `(name, commands_dir) → Result<()>` | Remove symlink |
| `prune_broken_commands` | `(commands_dir) → Result<usize>` | Remove broken `.md` symlinks |
| `check_command_install_status` | `(name, source_path, commands_dir) → SkillInstallStatus` | Return Installed / NotInstalled / Conflict |
| `migrate_commands_dir_quiet` | `(commands_link, tool_commands_target, central_commands, tool_key) → Result<(usize, Vec<String>)>` | Move `.md` files to store, create central symlinks, rename on conflict with `{tool_key}_{stem}.md`, delete original dir |

### `scan_all_sources()` change

For each source type (local, migrated, git repo), additionally call `scan_commands()` and populate `SourceGroup.commands` with install status checked against the central commands directory.

---

## 3. TUI Source View (`src/tui/source.rs`)

### `Category` enum

Add variant:

```rust
Commands,
```

### `ListRow` enum

Add variant:

```rust
CommandItem { group_index: usize, command_index: usize },
```

### UI presentation

- Category header: `"💬 Commands [installed/total]"`
- Display order: Skills → Agents → Commands
- New expansion state: `expanded_commands_sources: HashSet<usize>`
- Same status icons as agents: ✓ Installed (Green), ✗ Not Installed (DarkGray), ⚡ Conflict (Yellow)

### Operations

- `toggle_command()`: mirror `toggle_agent()`, call `install_command` / `uninstall_command`
- `execute_bulk_toggle()`: add `Category::Commands` branch
- `show_info()`: add `build_command_info_lines()` — display name, source, path, status, and `.md` content preview

---

## 4. TUI Tool View (`src/tui/tool.rs`)

### `CentralField` enum

Add variant `Commands` (after Agents, before Source).

### `LinkField` enum

Add variant `Commands` (directory link, `is_dir: true`).

### Central section

- New row for Commands path (editable, defaults to `~/.local/share/agm/commands`)

### Per-tool section

- Link items: add Commands row showing link status (✓/✗/broken/wrong/blocked)
- `toggle_link()`: add `LinkField::Commands` branch — same logic as Agents (directory link to central commands_source)
- `handle_blocked_link()`: add Commands migration path — call `migrate_commands_dir_quiet(link_path, agm_tools/{tool}/commands, central_commands, tool_key)`
- `recover_after_unlink()`: add Commands recovery — restore from `agm_tools/{tool}/commands/` or create empty dir

### Status calculation

- Tool overall link status computation includes commands link status

---

## 5. Status Command (`src/status.rs`)

### Per-tool output

Add `commands` row after `agents`:

```
  commands ✓ linked → ~/.copilot/commands
```

Shows: Linked / Missing / Broken / Wrong / Blocked (same as agents).

### Central summary

Add line:

```
Central commands : ~/.local/share/agm/commands (N installed)
```

Count installed commands from all source groups.

### `check` command

Include commands link in validation.

---

## 6. Testing

### Unit tests (`src/skills.rs`)

New tests using `tempfile` for isolation:

- `test_scan_commands`: create `commands/` with `.md` files, verify scan output
- `test_scan_commands_empty`: verify empty/missing dir returns empty vec
- `test_install_uninstall_command`: verify symlink creation and removal
- `test_install_command_conflict`: verify conflict detection
- `test_prune_broken_commands`: verify broken symlink cleanup
- `test_migrate_commands_dir_quiet`: verify migration, renaming, and central linking

### Integration tests

Existing CLI integration tests (`tests/cli.rs`) are unaffected. No new integration tests needed for this change.

---

## File Change Summary

| File | Changes |
|------|---------|
| `src/config.rs` | Add `commands_source` to CentralConfig, `commands_dir` to ToolConfig, update `default_config()` |
| `src/init.rs` | Add `commands_source` to `dirs_to_create` |
| `src/skills.rs` | Add `CommandInfo`, add `commands` to `SourceGroup`, add 6 new functions, update `scan_all_sources()` |
| `src/tui/source.rs` | Add `Commands` to Category/ListRow, add toggle/bulk/info logic |
| `src/tui/tool.rs` | Add `Commands` to CentralField/LinkField, add link/migrate/restore logic |
| `src/status.rs` | Add commands link display and central commands count |
