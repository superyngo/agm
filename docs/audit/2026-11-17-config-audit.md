# Config module audit — 2026-11-17

Status: Resolved (2026-11-17)

Scope: `src/config.rs`, `docs/reference/config.md`, and the paths they call into
(`src/linker.rs`, `src/platform.rs`, `src/tui/tool.rs` section editing). Triggered right after
commit `28aa280` (add `omp`, change the `agy` defaults). Quick-to-standard depth.

## Executive summary

- Health: **7/10**. The code is small, clippy is clean, and 24 unit tests in `config.rs` pass.
  One regression came from `28aa280`.
- Critical: 0 · High: 1 · Medium: 4 · Low: 7
- Top 3 priorities:
  1. **H1**: `agy` linking fails when `~/.gemini/config/` is missing.
  2. **M1**: no migration path for existing `agy` installs, so skills load twice.
  3. **M2**: Feature fields skip `resolve_path`, which contradicts the doc.

## Findings

### Architecture & design

#### 🔴 H1 — Nested Feature dirs fail when the parent is missing (regression from `28aa280`)
- Where: `src/linker.rs:138-145` and `:234-241` (`LinkStatus::Missing` → `do_link`).
  `src/platform.rs:94-95` calls `symlink` directly and never creates the parent. The new defaults
  `agy.skills_dir = "config/skills"` and `agents_dir = "config/agents"` are at `src/config.rs:184-185`.
- Evidence (real binary, temporary `$HOME`, `~/.gemini` present but no `config/`):
  `agm tool link` → `fail Failed to create link: …/.gemini/config/skills → …: No such file or directory`,
  exit 1. The `agents` Feature is never attempted.
- Impact: Antigravity CLI users who never ran the App (no `~/.gemini/config/` yet) get a hard
  failure on `agm tool link`.
- Recommendation: before `do_link` in the `Missing` branch, run
  `fs::create_dir_all(link_path.parent())`. Add an e2e test with a nested Feature dir.
  Effort: < 1 h.

#### 🟡 M1 — Changing `agy` defaults strands existing links; skills load twice
- Where: `src/config.rs:183-186`. The live machine still has `~/.gemini/GEMINI.md`,
  `~/.gemini/skills` and `~/.gemini/agents` symlinks, plus the old values in `~/.config/agm/config.toml:20-23`.
- Impact: if a user edits the config to the new values, AGM no longer tracks the old links. The CLI
  reads `~/.gemini/skills/` (top priority) **and** `~/.gemini/config/skills/`, so the same central
  skills get discovered twice.
- Recommendation: document the migration in `CHANGELOG.md` / `README.md`: run `agm tool unlink`,
  edit the config, then run `agm tool link`. Effort: < 1 h.

#### 🟡 M2 — Feature fields skip `ToolConfig::resolve_path`
- Where: `src/config.rs:308` uses `config_dir.join(relative)`. `docs/reference/config.md:70-71`
  calls `resolve_path` "the one resolver".
- Impact: `skills_dir = "~/x"` or `"$VAR/x"` becomes the literal `<config_dir>/~/x`. The settings,
  auth and mcp fields expand these correctly, so the behavior is inconsistent.
- Recommendation: use `self.resolve_path(relative)` in `resolve_link`, add a test, and update
  `config.md:65`. Effort: < 1 h.

### Code quality

#### 🟢 L3 — The two section-scan functions are duplicates and their header match is brittle
- Where: `src/config.rs:336-357` and `:360-388` contain the same start/end scan.
  The header match is `l.trim() == header`.
- Impact: these headers are not found: `[tools.omp]  # note`, `[tools."omp"]`, `[ tools.omp ]`.
  `replace_tool_section` also drops the trailing newline. Because the caller trims, the blank line
  before the next section is lost too (`src/tui/tool.rs:1613`).
- Recommendation: extract `section_bounds(text, key) -> Option<(usize, usize)>` and keep a
  trailing `\n`. Effort: ~1 h.

#### 🟢 L5 — Field names are matched as strings in two places
- Where: `src/config.rs:271-279` and `:300-306`.
- Recommendation: add one `fn field_value(&self, field) -> Option<&str>` and use it in both places.
  Effort: < 30 min.

### Security

#### 🟢 L4 — The collision guard only checks equality
- Where: `src/config.rs:322`.
- Impact: values such as `../elsewhere` are accepted and create a symlink outside `config_dir`.
  Existing real directories are still protected, because `LinkStatus::Blocked` skips them.
- Recommendation: refuse resolved paths that are not under `config_dir`. Effort: ~1 h.

#### 🟢 L7 — The drive-letter heuristic runs on every platform
- Where: `src/config.rs:262` (`bytes[1] == b':'`).
- Impact: a relative value like `a:b` counts as absolute on Unix. Minor.

No secrets in code. Auth files are only opened in the editor.

### Reliability

#### 🟡 M3 — `Config::save_to` is not atomic
- Where: `src/config.rs:105-106` uses `std::fs::write`. The TUI section editor already does
  temp-file + rename (`src/tui/tool.rs:1634-1637`).
- Impact: a crash or disk-full error mid-write truncates `config.toml`.
- Recommendation: share the temp-file + rename logic. Effort: < 1 h.

### Correctness of defaults

#### 🟡 M4 — `omp.mcp` is empty, but OMP keeps `mcp.json` in its config dir
- Where: `src/config.rs:229`. `~/.omp/agent/mcp.json` exists on this machine.
- Recommendation: after checking the OMP docs, set `mcp: vec!["mcp.json".into()]`. Also confirm
  that `SYSTEM.md` is what the user wants: in the Pi family it *replaces* the built-in system
  prompt rather than adding context. Effort: < 30 min.

#### 🟢 L6 — The live user config has `omp.auth = [""]`
- `~/.config/agm/config.toml:99`. It resolves to the config dir itself. This is a user-side edit,
  not a code change.

### Documentation

#### 🟢 L1 — `config.md:66-69` is stale
- The doc says `ToolState::get_group_files` resolves paths itself. It actually calls
  `tool.resolve_path` (`src/tui/tool.rs:1519`). Merge that text into the bullet on line 70.

#### 🟢 L2 — `omp` row out of order
- `config.md:92` puts `omp` after `pi`. The table is otherwise in alphabetical (BTreeMap) order.

### Testing

- `config.rs` has 24 tests, `cargo test` is green and `cargo clippy --all-targets` is clean.
- Gaps:
  - nested Feature dir with a missing parent (H1);
  - `~`/`$` in Feature fields (M2);
  - trailing newline and spacing in `replace_tool_section` (L3);
  - `save_to` failure behavior (M3).

### Performance

Nothing material. `resolve_link` canonicalizes on every call, and render paths go through
`StatusCache`.

## Prioritized action plan

1. Quick wins (< 1 day): H1, M2, M3, M4, L1, L2, L5. Add the M1 migration note.
2. Medium-term (1-5 days): L3 section-scan refactor, L4 containment guard, and the missing tests.
3. Long-term: none needed for this module.

## Metrics

- Files analyzed: 6 (`config.rs` 769 lines, `config.md` 107, plus `linker.rs`, `platform.rs`,
  `tui/tool.rs`, `paths.rs` call sites). Crate total: ~13.7k lines of Rust.
- Coverage: not measured (no coverage tool configured).
- Complexity hotspots: `resolve_link` (canonicalize fallback chain) and the section scan pair.
