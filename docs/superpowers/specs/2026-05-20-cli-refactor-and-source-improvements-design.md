# CLI Refactor and Source Improvements — Design

**Date:** 2026-05-20
**Status:** Approved by user, pending external review
**Scope:** Single implementation cycle (no sub-project split)

## Goals

1. Replace boolean flags on `agm tool` and `agm source` with explicit subcommands (breaking).
2. Add three new source operations: `del`, `rename`, and a `-n,--name` option for `add`.
3. Unify all stdout/stderr output from background operations in the source TUI through the existing `LogBuffer`, eliminating screen tearing.
4. Display preload character counts (skills: YAML frontmatter `name`+`description`; agents/commands: whole-file) in the source TUI info popups, with rollup totals at repo and category levels.

## Non-Goals

- No changes to the tool TUI or tool linker behavior.
- No global logging framework; the existing `LogBuffer` is sufficient.
- No transition aliases for removed flags — this is a clean break on a 0.x version.
- No async/concurrent git operations.
- No new YAML crate; frontmatter parsing is hand-rolled.

---

## 1. CLI Restructure (breaking)

### 1.1 New shape

```text
agm init
agm tool                          # TUI
agm tool link
agm tool unlink
agm tool status
agm source                        # TUI
agm source add <source> [-n <name>] [--all]
agm source update
agm source list
agm source del <target>
agm source rename <old> <new>
agm --config <path> ...           # unchanged, global
agm -v | --version                # unchanged
agm tool --help                   # unchanged
agm source --help                 # unchanged
```

### 1.2 clap derive layout

```rust
#[derive(Subcommand)]
enum Commands {
    Init,
    Tool { #[command(subcommand)] action: Option<ToolAction> },
    Source { #[command(subcommand)] action: Option<SourceAction> },
}

#[derive(Subcommand)]
enum ToolAction { Link, Unlink, Status }

#[derive(Subcommand)]
enum SourceAction {
    Add {
        source: String,
        #[arg(short = 'n', long)] name: Option<String>,
        #[arg(long)] all: bool,
    },
    Update,
    List,
    Del { target: String },
    Rename { old: String, new: String },
}
```

`action: None` for `Tool` and `Source` continues to launch the TUI (existing behavior preserved).

### 1.3 Dispatch table

| Subcommand | Calls |
|---|---|
| `agm tool link` | existing `link_all` |
| `agm tool unlink` | existing `unlink_all` |
| `agm tool status` | existing `status::status` |
| `agm source add <s> [-n] [--all]` | `clone_or_pull` or `add_local_copy` with a stdout-printing `CloneProgress` sink, then `install_skill` / `install_agent` per existing flow |
| `agm source update` | `update_all_with_progress` with a stdout-printing callback that prints `RepoStart` / `RepoComplete` / `AllDone` events (replaces the retired `update_all`) |
| `agm source list` | existing list rendering |
| `agm source del <t>` | `resolve_source_target` → `delete_source` |
| `agm source rename <o> <n>` | `rename_source` with stdout sink |

### 1.4 Removed surface

- `agm tool -l/--link`, `-u/--unlink`, `-s/--status`
- `agm source -a/--add`, `-u/--update`, `-l/--list`
- Mutual-exclusivity check `flag_count > 1` in `main.rs` `Commands::Tool` arm (subcommands are naturally exclusive)

### 1.5 Migration

- CHANGELOG `Unreleased` notes BREAKING.
- README examples and any text in `RELEASE.md` referring to old flags updated.
- `gpinstall.sh` is an external gist not modified here.

---

## 2. New Source Subcommands

### 2.1 `source add -n,--name <name>`

Override the directory name used when cloning/copying a source.

- Signature change: `skills::clone_or_pull(url, source_dir, target_name: Option<&str>, on_progress)`.
- Signature change: `skills::add_local_copy(source_path, source_dir, target_name: Option<&str>, on_progress)`.
- When `target_name` is `Some(n)`:
  - Validate `n`: non-empty, no `/` or `\`, no `..`, not `.`. Reject with error otherwise.
  - For URL sources: `n` replaces `repo_name_from_url(url)`; clone target becomes `source_dir/{n}/`. `.git` suffix handling is irrelevant because the name is user-supplied.
  - For local sources: `n` replaces the basename; copy target becomes `source_dir/local/{n}/` (the `local/` prefix is preserved).
- Existing "directory exists but belongs to a different remote" guard (URL path) and "destination already exists" guard (local path) still apply against the resolved name — the override does not bypass them.

### 2.2 `source del <target>`

`target` resolves in this order:

1. Exact match against a directory name under `source_dir` (works for both git-cloned and local sources; for local sources under `source_dir/local/<name>/`, the match is on `<name>`).
2. Normalized git URL match against any repo's `origin`. Local sources have no git origin and are not considered in this step.

If 0 matches → list available names and exit 1.
If multiple matches (e.g. two repos cloned from the same URL into different dirs, when `target` is the URL) → list and exit 1; user must disambiguate by name.

On match: reuse existing `skills::delete_source(group, skills_dir, agents_dir, commands_dir)`.

#### New resolver

```rust
pub fn resolve_source_target(
    target: &str,
    source_dir: &Path,
    skills_dir: &Path,
    agents_dir: &Path,
    commands_dir: &Path,
) -> anyhow::Result<SourceGroup>;
```

Internally runs `scan_all_sources(...)` once and filters the resulting `Vec<SourceGroup>` per the rules above. Returns the matched `SourceGroup` by value (cloned). On 0 / >1 matches, returns an `Err` whose message lists the available source names. The CLI dispatch in `main.rs` calls this resolver then hands the result to `delete_source`.

### 2.3 `source rename <old> <new>`

- `old`: resolves via `resolve_source_target` (§2.2).
- `new`: validate the same way as `-n` (`no /`, `no \`, `no ..`, non-empty, not `.`).
- If `source_dir/<new>` (or `source_dir/local/<new>` when the source is local) already exists → error and stop (no overwrite).

#### Signature

```rust
pub struct RenameReport {
    pub skills_relinked: usize,
    pub agents_relinked: usize,
    pub commands_relinked: usize,
    pub rollback_failures: Vec<String>, // empty on the happy path
}

pub fn rename_source(
    old: &str,
    new: &str,
    source_dir: &Path,
    skills_dir: &Path,
    agents_dir: &Path,
    commands_dir: &Path,
    mut on_progress: impl FnMut(CloneProgress),
) -> anyhow::Result<RenameReport>;
```

#### Procedure

1. Resolve `old` to a `SourceGroup`; snapshot installed skills/agents/commands (names only).
2. For each installed item, call existing `uninstall_skill` / `uninstall_agent` / `uninstall_command` to remove central symlinks.
3. `fs::rename(source_dir/old, source_dir/new)` (or the `local/` variant for local sources). On failure: best-effort rollback — attempt to re-install the items uninstalled in step 2 against the original path; any items that fail to re-link are collected into `RenameReport.rollback_failures` and surfaced in the returned error. This is an accepted risk for a rare failure mode; the function does not attempt a second-level rollback.
4. Re-scan the renamed dir and re-install the previously-installed items (preserves blocklist semantics — items not previously installed stay not-installed).
5. Emit a final `CloneProgress::Done` event with a summary: "Renamed `old` → `new`; relinked N skills, M agents, K commands."

Rationale for un/re-install over symlink target rewrite: cross-platform symlink retargeting is fragile (Windows junctions, hardlinks) and the existing un/install functions already handle every edge case.

---

## 3. Source TUI: Unified Logging

### 3.1 Root cause

`skills::clone_or_pull` shells out with `Command::new("git").status()` — child inherits stdio, prints directly to the terminal, tears the TUI frame. The same pattern appears in the legacy CLI `update_all` (the path that backs `agm source --update` today). The newer `update_all_with_progress` already pipes stdio and uses a callback; the legacy path will be retired (see §3.5).

### 3.2 Callback API (Plan A, mirrors existing `update_all`)

New types in `src/skills.rs`:

```rust
pub enum CloneAction { Clone, Pull }

pub enum CloneProgress {
    Start  { name: String, url: String, action: CloneAction },
    GitLine { line: String, is_err: bool },
    Done   { name: String, success: bool, message: String },
}
```

Refactored signature:

```rust
pub fn clone_or_pull(
    url: &str,
    source_dir: &Path,
    target_name: Option<&str>,
    mut on_progress: impl FnMut(CloneProgress),
) -> anyhow::Result<(PathBuf, Vec<(String, PathBuf)>)>
```

Implementation:

- `Command::new("git").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()`.
- Spawn two reader threads (stdout, stderr), each reading line-by-line into an `mpsc::Sender<CloneProgress>`.
- Main thread drains the receiver, calling `on_progress` for each `GitLine`.
- Join threads, wait for child exit, emit final `Done`.
- Synchronous: function still blocks until git finishes.

### 3.3 `add_local_copy` symmetry

```rust
pub fn add_local_copy(
    source_path: &Path,
    source_dir: &Path,
    target_name: Option<&str>,
    mut on_progress: impl FnMut(CloneProgress),
) -> anyhow::Result<(PathBuf, Vec<(String, PathBuf)>)>
```

Emits `Start { action: Clone, url: source_path.display() }` and `Done` only (no `GitLine`). Keeps the call sites uniform.

### 3.4 Caller migrations

**`src/main.rs` (CLI `source add`):** sink prints each `GitLine` to stdout as-is, prints `Start`/`Done` lines colored like today.

**`src/tui/source.rs::do_add_submit`:** sink pushes each progress event into `self.log` with the appropriate `LogLevel` (Error for `is_err: true` lines or failed `Done`, Success for successful `Done`, Info otherwise). No screen redraw is forced from inside the sink — the next event-loop tick will redraw naturally.

### 3.5 Other `println!` removal in `skills.rs`

Retire the legacy `update_all(skills_dir, agents_dir, source_dir)` (the variant that uses `git`'s inherited stdio and direct `println!`). The CLI `agm source update` dispatch is repointed to `update_all_with_progress`, supplying a stdout-printing callback (see §1.3 dispatch table). After repointing, `update_all` is deleted.

All remaining direct `println!` calls inside `skills.rs` (inside `clone_or_pull`, `migrate_tool_dir`, etc.) are removed: the affected helpers either take a callback or return structured data so the caller prints. No skill helper writes to stdout itself after this change.

### 3.6 TUI rename binding

- `KeyCode::F(5)` → existing `refresh()` (moved from `r`).
- `KeyCode::Char('r')` → enter rename mode (only when the current row is a `SourceHeader`; otherwise show status hint "Rename: select a source row first").
- New fields on the app state:
  - `rename_mode: bool`
  - `rename_input: String`
  - `rename_cursor: usize`
  - `rename_target_group_index: Option<usize>` (None when no row selected; defensive against state-out-of-sync)
- Input rendering mirrors the existing add input bar.
- On submit: call new helper `skills::rename_source` (signature in §2.3). The sink pushes each `CloneProgress` event into `self.log`. On error, status bar shows the error message; partial-rollback failures (if any) are listed in the log per §2.3 step 3.

### 3.7 Footer/help update

All footer hint strings and `?` help popup text in `tui/source.rs` updated to show `r:rename  F5:refresh` (and any other affected keys). No other keys change.

---

## 4. Preload Character Counts

### 4.1 New helpers in `src/skills.rs`

```rust
/// Sum of unicode chars in the `name` and `description` values of a SKILL.md
/// YAML frontmatter. Returns 0 if the file is missing, has no frontmatter,
/// or neither key is present.
pub fn skill_preload_chars(skill_path: &Path) -> usize;

/// Total unicode-char count of the entire file. Returns 0 on read error.
pub fn file_char_count(path: &Path) -> usize;
```

### 4.2 Frontmatter parser (hand-rolled, ~40 LOC)

Rules:

1. File must start with a line that is exactly `---` (trimmed). Otherwise no frontmatter.
2. Frontmatter ends at the next line that is exactly `---` or `...` (trimmed). If no terminator found in the first 200 lines, no frontmatter.
3. Inside the frontmatter, find keys `name:` and `description:` at column 0 (not indented — top-level keys only).
4. Value extraction per key:
   - Inline scalar (`key: value` or `key: "value"` / `key: 'value'`): strip outer quotes if matched, trim whitespace, count chars.
   - Block scalar (`key: >` or `key: |` on the key line, value on subsequent indented lines): collect indented continuation lines until a non-indented non-empty line or frontmatter end. Concatenate with `\n` (preserving join behavior is unnecessary for char count); count chars.
5. Total = chars(name_value) + chars(description_value).

Note: this is intentionally lossy on edge cases (anchors, aliases, multi-line flow scalars). Skills using exotic YAML will under-count; that is acceptable for an info display.

### 4.3 Eager computation

The fields are populated during `scan_all_sources` (the existing scan path that produces `SourceGroup`s for the TUI list and `list` command). The relevant types are `SkillInfo`, `AgentInfo`, `CommandInfo` (defined in `src/skills.rs` around lines 22/30/38):

- `SkillInfo { ..., preload_chars: usize }`
- `AgentInfo { ..., preload_chars: usize }`
- `CommandInfo { ..., preload_chars: usize }`

`preload_chars` is computed **inline** during each `*Info` struct's construction inside `scan_all_sources`, using the `source_path` already available there (the `(name, PathBuf)` pairs returned by `scan_skills` / `scan_agents` / `scan_commands`). One scan, one read per file.

Re-scan triggers (`F5` refresh in TUI, after add/del/rename) recompute. The CLI `list` path also gets the data even though it does not display it (cheap; keeps a single scan API).

### 4.4 Display

**Skill info popup** (`build_skill_info_lines`): after the existing `Status:` line, add:
```
Preload chars: <n>
```

**Agent info popup** / **Command info popup**: after `Status:`, add:
```
Char count: <n>
```

**Source header info** (`build_source_info_lines`): new section before the existing detail block. All values are summed `preload_chars` across items in that status bucket (not item counts):
```
Preload chars:
  Skills    — installed <chars_i>  not-installed <chars_u>
  Agents    — installed <chars_i>  not-installed <chars_u>
  Commands  — installed <chars_i>  not-installed <chars_u>
```
Rows for categories with zero items in this source are omitted.

**Category header info** (`build_category_info_lines`): append:
```
Total preload chars:
  installed:     <X>
  not-installed: <Y>
```
Summed across every `SourceGroup` for that category. Conflict items are counted as not-installed (per §4.5).

### 4.5 Conflict status handling

Items with `install_status == Conflict` are counted under **not-installed** for these totals. (Rationale: a conflict link is not pointing at this source's file, so it does not contribute that source's preload chars to the runtime.)

---

## 5. Files Touched

| File | Change |
|---|---|
| `src/main.rs` | CLI restructure; new `del`/`rename` dispatch; CLI sink for `add` progress |
| `src/skills.rs` | `clone_or_pull` + `add_local_copy` callback-ized with `target_name` param; new `resolve_source_target` resolver (§2.2); new `rename_source` (§2.3); new `skill_preload_chars` + `file_char_count`; `preload_chars` field on `SkillInfo`/`AgentInfo`/`CommandInfo`; legacy `update_all` removed; remaining internal `println!`s removed |
| `src/tui/source.rs` | `do_add_submit` consumes new callback; rename mode + rebound `r`/`F5`; info popups show char counts |
| `tests/` | New integration tests for CLI subcommands; new unit tests for frontmatter parser, rename behavior, name validation |
| `CHANGELOG.md` | Unreleased entry covering: breaking CLI; `del`/`rename`/`--name`; TUI log unification; preload char stats |
| `README.md` | All CLI examples updated |

Not touched: `linker.rs`, `platform.rs`, `config.rs`, `init.rs`, `status.rs`, `editor.rs`, `paths.rs`, `tui/tool.rs`, `tui/log.rs` (sufficient as-is), `tui/popup.rs`, `tui/background.rs`, `tui/mod.rs`.

---

## 6. Test Plan

### 6.1 CLI parsing (assert_cmd in `tests/`)

- `agm tool link` / `unlink` / `status` parse and dispatch.
- `agm tool` with no action launches TUI (smoke: exit code path).
- `agm source add <url>` works without `-n`.
- `agm source add <url> -n custom` accepts the name.
- `agm source add <path> -n bad/name` rejects the name (validation error).
- `agm source del <name>` and `agm source rename <old> <new>` parse.
- Old flags (`-l/-u/-s/-a/--add/--update/--list`) are rejected.

### 6.2 Source operations (unit, with `tempfile`)

- `skill_preload_chars`:
  - Standard `name:` + `description:` keys.
  - Quoted values (`"..."`, `'...'`).
  - Block scalar `description: |` and `>` with continuation.
  - Missing frontmatter → 0.
  - Missing key → 0 for that key.
  - File missing → 0.
- `rename_source`:
  - Successful rename relinks installed items, leaves blocklisted items still blocklisted.
  - Target dir exists → error, no state mutation.
  - Invalid `new` name → error, no state mutation.
- Name validation: `/`, `\`, `..`, `.`, empty all rejected.
- `del` target resolver: by name, by URL, no match, multi match.

### 6.3 TUI log sink (unit-level where feasible)

- Mock the `on_progress` callback for `clone_or_pull` against a deliberately broken URL; verify that `GitLine { is_err: true }` events are produced and that the function does not write to process stdout (capture stdout in test, assert empty for the git output portion).

### 6.4 Manual smoke

- Run `agm source` TUI; press `a`, paste a real repo URL; confirm no screen tearing and log popup shows git output.
- In TUI, press `r` on a repo row, type a new name, submit; confirm rename + relink.
- Press `F5` to refresh — confirm prior `r:refresh` behavior moved.

---

## 7. Open Risks

- Hand-rolled YAML parser will mis-handle exotic frontmatter; acceptable because the count is informational.
- Two reader threads + channel in `clone_or_pull` add minor complexity; bounded by `git`'s output volume which is small.
- Windows: `Command::spawn` + piped stdout behaves the same as on Unix; tested implicitly through existing `update_all` which already pipes.

---

## 8. Out of Scope (deferred)

- Renaming individual skills inside a repo (only repo folder rename here).
- Bulk operations (`source del --all`, etc.).
- Showing token counts (would require a tokenizer; chars are a proxy).
- Tool TUI improvements.
