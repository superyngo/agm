# Skills Architecture Refactor — Design Spec

## Problem Statement

AGM's skills management has overlapping responsibilities between `agm link` and `agm skills`, and lacks interactive management capabilities. The current architecture:

1. `agm link` handles both tool↔central linking AND skill source linking — these concerns should be separated
2. `agm skills add` with multi-skill repos installs everything silently — users need selective installation
3. Local directory sources are symlinked in-place rather than managed centrally like repos
4. `agm skills list` is a flat list — no grouping by source, no install status
5. `agm skills remove` only removes one skill at a time — no interactive bulk management
6. No link health maintenance during list/manage operations

## Approach

**Incremental refactor (Approach A):** Add `src/manage.rs` for TUI, refactor `skills.rs` for new data model, adjust `main.rs` routing. Minimal impact on unrelated modules.

## Data Model

### New Types in `skills.rs`

```rust
/// Installation status of a skill (central link state)
pub enum SkillInstallStatus {
    Installed,      // central skills dir has symlink pointing to this skill's source
    NotInstalled,   // source exists but no central link
    Conflict,       // another skill with the same name is already installed from a different source
}

/// Full info about a single skill
pub struct SkillInfo {
    pub name: String,
    pub source_path: PathBuf,       // absolute path in source_dir
    pub install_status: SkillInstallStatus,
}

/// What kind of source this is
pub enum SourceKind {
    Repo { url: Option<String> },   // git-cloned repository (URL from config or git remote lookup)
    Local,                          // copied local directory (source_dir/local/{name}/)
    Migrated { tool: String },      // migrated from tool (source_dir/agm_tools/{tool}/)
}

/// A source and all skills it contains
pub struct SourceGroup {
    pub name: String,               // display name
    pub kind: SourceKind,
    pub path: PathBuf,              // absolute path under source_dir
    pub skills: Vec<SkillInfo>,
}
```

### Notes on `SourceKind::Repo` URL Resolution

The `url` field is `Option<String>` because the reverse lookup from directory name to URL can fail (e.g., directory manually placed, or repo removed from config). Resolution order:
1. Match directory name against `config.skill_repos` via `repo_name_from_url()`
2. Fallback: `git -C {path} remote get-url origin`
3. If both fail: `None`

## Responsibility Separation

| Command | Responsibility |
|---------|---------------|
| `agm link` | Tool ↔ central links (prompt, skills directory, files) + first-time skill migration + skill_repos clone/pull |
| `agm skills add` | Add source (clone repo / copy local dir) → scan skills → multi-select install |
| `agm skills manage` | Interactive TUI (toggle install/uninstall, delete source, edit SKILL.md) |
| `agm skills list` | Read-only list grouped by source with install status |
| `agm skills update` | Git pull all repos → prune broken links |

`agm skills remove` is replaced by `manage`, but kept as a **hidden clap alias** that prints a migration message: `"'agm skills remove' has been replaced by 'agm skills manage'. Use 'agm skills manage' to interactively install/uninstall skills."` Remove the alias in a future release.

## Detailed Design

### 1. `agm skills add` — Multi-Select Installation

#### Local Directory Handling (New Flow)

```
agm skills add /some/local/path
  → detect: not a URL → local path
  → scan path for SKILL.md files FIRST (before copying)
  → if no skills found → error: "No skills found at {path}. A skill must contain a SKILL.md file."
  → determine source name (dirname, or skill name if single skill)
  → if source_dir/local/{source_name}/ already exists → error with suggestion
  → copy entire directory to source_dir/local/{source_name}/
  → scan copied directory for skills → enter multi-select flow
```

Original files are preserved (copy, not move). Scanning happens **before** copying to avoid creating useless directories when no skills are found.

#### Multi-Select Flow

```
agm skills add <source> [--all/-a]
```

- If 1 skill found → install directly (no prompt)
- If N > 1 skills found:
  - With `--all/-a` flag → install all silently
  - Without flag → show `dialoguer::MultiSelect` list:
    - All skills pre-selected by default
    - User deselects unwanted skills
    - Selected skills get central symlinks created

#### CLI Change

```
Skills subcommand:
  agm skills add <source> [--all/-a]
    source: git URL or local path
    --all/-a: skip multi-select, install all skills
```

### 2. `agm skills list` — Grouped by Source

#### Scan Logic: `scan_all_sources()`

```
fn scan_all_sources(source_dir: &Path, skills_dir: &Path, skill_repos: &[String]) -> Vec<SourceGroup>
```

**Graceful degradation:** If `source_dir` does not exist, return empty `Vec`. If `local/` or `agm_tools/` subdirs are absent, skip them silently.

1. Read all entries under `source_dir`
2. Classify each:
   - `local/` → iterate subdirs, each is `SourceKind::Local`
   - `agm_tools/` → iterate subdirs by tool key, each is `SourceKind::Migrated`
   - Others → `SourceKind::Repo` (resolve URL: match against `config.skill_repos`, fallback to `git remote get-url origin`)
3. For each source, `scan_skills()` to find skills
4. For each skill, check central `skills_dir/{name}`:
   - Symlink exists and points to this skill → `Installed`
   - Symlink exists but points to a **different** source → `Conflict`
   - No symlink → `NotInstalled`
5. Return `Vec<SourceGroup>` sorted alphabetically

#### Prune Before Display

Call `prune_broken_skills(skills_dir)` before scanning. Report count in summary if any were pruned.

#### Output Format

```
📦 my-skill-repo (repo: https://github.com/user/my-skill-repo.git)
   ✓ skill-alpha          installed
   ✗ skill-beta           not installed
   ✓ skill-gamma          installed

📁 my-local-skills (local)
   ✓ custom-skill         installed

📁 agm_tools/claude (migrated from claude)
   ✓ old-claude-skill     installed

── Summary ──────────────────────────
3 sources, 5 skills (4 installed, 1 not installed)
```

- `✓` green = installed, `✗` dim/gray = not installed, `⚡` yellow = conflict (name used by another source)
- Broken links already pruned, won't appear

### 3. `agm skills manage` — Interactive TUI

#### Entry Points

```
agm skills manage                    → sub-menu: [all], source1, source2, ...
agm skills manage <source_name>      → directly open that source
agm skills manage all                → skip sub-menu, show all sources
```

If no argument and only one source exists, skip sub-menu.
If **zero sources** exist, print: `"No skill sources found. Use 'agm skills add' to add a source."` and exit without launching TUI.

#### TUI Layout (ratatui + crossterm)

```
╭─ AGM Skills Manager ──────────────────────────────────╮
│                                                        │
│ 📦 my-skill-repo (repo)                     [3 skills]│
│ > ✓ skill-alpha              installed                 │  ← cursor
│   ✗ skill-beta               not installed             │
│   ✓ skill-gamma              installed                 │
│                                                        │
│ 📁 my-local-skills (local)                   [1 skill] │
│   ✓ custom-skill             installed                 │
│                                                        │
├────────────────────────────────────────────────────────┤
│ ␣ toggle  e edit  Del remove source  i info            │
│ r refresh  / search  q quit                            │
╰────────────────────────────────────────────────────────╯
```

#### Keybindings

| Key | On Skill Row | On Source Header Row |
|-----|-------------|---------------------|
| `↑` / `↓` | Move cursor | Move cursor |
| `PgUp` / `PgDn` | Page jump | Page jump |
| `Space` | Toggle install/uninstall (immediate) | Toggle all skills in this source |
| `e` | Suspend TUI → open SKILL.md in editor → resume TUI | No-op |
| `Del` / `d` | No-op | Confirm dialog: `Delete "{name}" and N skills? [y/N]` |
| `i` | Show skill source path in status bar | Show source URL/path in status bar |
| `r` | Re-scan filesystem, refresh list | Same |
| `/` | Enter search mode (filter by skill name) | Same |
| `q` / `Ctrl+C` | Exit | Exit |
| `Esc` | Clear search filter / clear info | Same |

#### Behavior Details

**Space toggle (skill):**
- Installed → remove central symlink → status changes to "not installed"
- Not installed → create central symlink → status changes to "installed"
- Status bar shows brief confirmation (e.g., `✓ skill-alpha installed`) for ~2 seconds
- All operations are immediate — no save/confirm step

**Space toggle (source header):**
- If all skills installed → uninstall all
- Otherwise → install all
- Same immediate behavior as individual toggle

**Del on source header:**
- Inline confirmation prompt at bottom: `Delete "{name}" and 3 skills? [y/N]`
- **For migrated sources** (`agm_tools/`): double confirmation with stronger warning:
  `"⚠ WARNING: Migrated skills from {tool} were moved (not copied) during initial setup. Deleting is PERMANENT and UNRECOVERABLE. Type 'delete' to confirm: "`
- On confirm:
  1. Remove all central symlinks for skills in this source
  2. Delete source directory from source_dir
  3. If repo, remove URL from config.skill_repos and save config
  4. Refresh display

**Editor (e key):**
1. Suspend TUI (restore terminal to normal mode)
2. Launch `$EDITOR` (or config editor, or vi) with SKILL.md path
3. Wait for editor to exit
4. Re-enter TUI (re-initialize terminal raw mode)

**Terminal safety:** Install a panic hook at TUI startup that calls `crossterm::terminal::disable_raw_mode()` and `crossterm::execute!(stdout, LeaveAlternateScreen)` to ensure terminal is restored even on panic or editor crash.

**Search (/ key):**
- Status bar becomes text input field
- Keystrokes filter visible skills by name (case-insensitive substring)
- Source headers remain visible if any of their skills match
- Empty sources hidden during search
- `Esc` clears filter and shows all
- `Enter` confirms filter (stays filtered)

### 4. Link Health Maintenance

#### When

| Command | Timing |
|---------|--------|
| `agm skills list` | Before scanning |
| `agm skills manage` | Before entering TUI + on each `r` refresh |
| `agm skills update` | After git pull (existing behavior, kept consistent) |

#### What

`prune_broken_skills(skills_dir)`:
- Scan central skills directory for symlinks
- If symlink target does not exist → remove symlink
- Return count of pruned links
- Silent operation; count displayed in summary (list) or status bar (manage)

#### What It Does NOT Do

- Does NOT auto-rebuild missing links (respects user's install/uninstall choices)
- Does NOT modify source directories

### 5. `agm link` Changes

#### What stays

- Tool ↔ central skills directory symlink (the directory-level link)
- Tool ↔ central prompt file symlink
- Tool-managed files linking
- First-time skill migration (move tool's existing skills to source_dir/agm_tools/{tool}/)
- skill_repos clone/pull during link

#### What's removed

Nothing is removed from `agm link` — it keeps its current behavior. The separation is:
- `agm link` creates/manages the **directory-level** symlink (tool's `skills/` → central `skills/`)
- `agm skills add/manage` creates/manages the **individual skill** symlinks (central `skills/{name}` → `source/{repo}/{name}`)

These are two different levels of linking that don't overlap.

## File Changes Summary

| File | Change Type | Description |
|------|------------|-------------|
| `Cargo.toml` | Modify | Add `ratatui`, `crossterm` dependencies |
| `src/manage.rs` | **New** | TUI interface — ratatui app, event handling, rendering |
| `src/skills.rs` | Refactor | New data types (`SkillInfo`, `SourceGroup`, etc.), `scan_all_sources()`, `install_skill()`, `uninstall_skill()`, `delete_source()`, `add_local` → copy to `source_dir/local/`, remove `remove_skill()` |
| `src/main.rs` | Modify | `SkillsAction::Remove` → `SkillsAction::Manage` (keep `Remove` as hidden alias with migration message), add `--all/-a` flag to `Add`, multi-select with `dialoguer::MultiSelect`, route `manage` to new module |
| `src/config.rs` | Modify | Add `remove_skill_repo(&mut self, url: &str)` method for `delete_source()` to use |
| `src/status.rs` | Modify | `skills list` output uses `scan_all_sources()` for grouped display |

### Unchanged Files

`linker.rs`, `platform.rs`, `files.rs`, `paths.rs`, `editor.rs`, `init.rs`

## Key Function Signatures

```rust
// skills.rs — new public API
pub fn scan_all_sources(source_dir: &Path, skills_dir: &Path, skill_repos: &[String]) -> Vec<SourceGroup>;
pub fn install_skill(name: &str, source_path: &Path, skills_dir: &Path) -> anyhow::Result<()>;
pub fn uninstall_skill(name: &str, skills_dir: &Path) -> anyhow::Result<()>;
pub fn delete_source(group: &SourceGroup, skills_dir: &Path) -> anyhow::Result<()>;
// add_local refactored: copy to source_dir/local/, return skill list
pub fn add_local_copy(source: &Path, source_dir: &Path) -> anyhow::Result<(PathBuf, Vec<(String, PathBuf)>)>;
// add_from_url refactored: clone/pull, return skill list (no auto-install)
pub fn clone_or_pull(url: &str, source_dir: &Path) -> anyhow::Result<(PathBuf, Vec<(String, PathBuf)>)>;

// config.rs — new method
impl Config {
    pub fn remove_skill_repo(&mut self, url: &str);
}

// manage.rs — TUI entry point
pub fn run(config: &mut Config, source_filter: Option<&str>) -> anyhow::Result<()>;
```

## Duplicate Source Handling

- **Same repo URL added twice:** `config.add_skill_repo()` already deduplicates. Clone detects existing dir and does `git pull` instead.
- **Different URLs resolving to same repo name:** `clone_or_pull()` checks if `source_dir/{repo_name}` exists AND belongs to a different remote URL; errors clearly with suggestion to rename.
- **Same local dir added twice:** `add_local_copy()` checks if `source_dir/local/{name}` already exists; errors with message.

## Testing Strategy

- **`skills.rs` unit tests**: Test `scan_all_sources()`, `install_skill()`, `uninstall_skill()`, `delete_source()` with tempdir fixtures
- **`manage.rs`**: Manual TUI testing (ratatui apps are hard to unit test); extract logic into testable helper functions where possible
- **Integration tests**: `agm skills add`, `agm skills list` with assert_cmd
- **Existing tests**: Must continue to pass after refactor
