# Code Audit Report — agm v0.16.0 (+1 docs commit)
Status: Mostly resolved — B13–B50 fixed except B40 (part), B43–B46 (structural remainder), see [BACKLOG](../plan/BACKLOG.md)
Date: 2026-09-29 · Scope: `src/`, `tests/`, `Cargo.toml` · Focus: implementation quality, clarity,
integrity, simplicity · Excludes BACKLOG B1–B12 (already tracked) except where a finding widens one.

Evidence legend: **✅ reproduced** on `target/debug/agm` in a throwaway `$HOME` · **✔ verified** by
reading the code · **◦ reported** by a module scout, plausible, not independently re-read.
Line numbers are from commit `7022991`.

## Executive Summary

- **Health: 6/10.** Clean default clippy, 194 passing tests, good docs discipline. But the two
  things the architecture promises most loudly — *never destroy silently* and *state is derived* —
  are broken in places, and the CLI and TUI implement the same link/unlink flows twice with
  different behavior.
- **Critical/High: 9** (3 reproduced on the binary).
- **Top 3 priorities**
  1. **C1** `agm tool link` migration permanently deletes tool content it does not recognize (✅).
  2. **C2/C3** Unlink is two different operations: CLI leaves symlinks into the store (✅,
     contradicts `cli.md`), TUI restores only migrated items. Pick one semantic, one function.
  3. **S1/S3** Structural root cause: Skill/Agent/Command and the four Features are hand-copied
     3–4×, and CLI/TUI each own their orchestration. "Commands was forgotten" recurs in ≥6 places
     (B2, B3, H3, L1, M12, C3's skip list).

---

## Findings by Category

### Integrity & Correctness

#### 🔴 Critical
- **C1 ✅ Migration deletes unrecognized tool content and the tool's copy of conflicting items.**
  `src/skills.rs:1209` `migrate_tool_dir_quiet` ends with `fs::remove_dir_all(skills_link)`; same
  pattern in `migrate_agents_dir_quiet` (~`:1289`) and `migrate_commands_dir_quiet` (~`:1375`).
  Anything `scan_skills` doesn't pick up (dirs without `SKILL.md`, `README.md`, skills deeper than
  depth 3) and any item hitting the `"already in store, re-linking"` branch is destroyed.
  Repro: `~/.claude/skills/{real-skill/SKILL.md, notes-dir/important.txt, README.md, dup/SKILL.md}`
  with `dup` pre-existing in `agm_tools/claude/` → `agm tool link` reports "Migrated 2 skill(s)";
  `important.txt`, `README.md`, and the tool's edited `dup/SKILL.md` exist nowhere afterwards.
  - Impact: silent user data loss on first adoption; violates the architecture invariant.
  - Fix: after moving recognized items, `fs::remove_dir` (empty only); if anything remains, rename
    the directory to `<dir>.agm-<ts>.bak` and report it. On `dest.exists()`, keep the tool copy as a
    `.bak` unless contents are identical. Add a regression test first. Effort **M**.
    (B1 covers the separate `link_all` agents-branch delete; fix both with the same helper.)

#### 🔴 High
- **C2 ✅ `agm tool unlink` "copies back" symlinks, not content.** `src/main.rs:388`
  `unlink_all` → `src/skills.rs:1385` `copy_dir_all`, which recreates links. Central-store skills
  are links into `source/…`, so the tool ends up with links into AGM's data dir. `cli.md` says
  "the tool ends up with real copies"; architecture says unlink keeps the tool working "after AGM
  steps out". Repro: after unlink, `~/.claude/skills/real-skill -> …/agm/source/agm_tools/claude/real-skill`.
  - Fix: a dereferencing copy for unlink (`copy_dir_resolved`); keep link-preserving copy only
    where it is actually wanted (or delete it if unused). Effort **S**.
- **C3 ✔ CLI and TUI unlink have contradictory semantics.** TUI `recover_after_unlink`
  (`src/tui/tool.rs:1003–1219`) moves *migrated* items back from `agm_tools/<tool>`; if nothing was
  migrated it creates an **empty** dir. CLI copies the whole store. Inside the TUI path: renames are
  `.is_ok()`-swallowed (EXDEV across volumes loses the restore silently, `:1047/1086/1141`),
  `dest.exists()` skips silently, and the skills loop skips `agents` but not `commands` (`:1033`).
  - Fix: decide the semantic (restore pre-AGM state vs detach with copies), implement once in the
    domain layer, call from both; document in `cli.md`/`tui.md`. Effort **M**.
- **H1 ✔ Source Manager keys UI state by `usize` group index across rescans.**
  `src/tui/source.rs:99–101` `expanded_*_sources: HashSet<usize>`, `ConfirmState { group_index }`
  (`:44–70`), `rename_target_group_index`. `refresh()` (`:332`) re-sorts `groups` without remapping.
  A background add finishing while a delete/rename confirmation is open can retarget it to a
  different source (◦ scenario; fields and missing remap ✔).
  - Fix: key by source path (or name); resolve to index at use time. Effort **M**.
- **H2 ✔ Background task can wedge forever.** `src/tui/background.rs:55` `poll` loops on
  `try_recv` and ignores `Disconnected`; a panicking worker leaves `is_running = true`, blocking all
  future update/add with "already in progress". Fix: handle `Disconnected` → clear state, log error.
  Effort **XS**.
- **H3 ✔ Uninstalled agents/commands come back on update.** `uninstall_agent`/`uninstall_command`
  (`src/skills.rs:301–320`) never blocklist; `update_all_with_progress` (`:645–667`) checks the
  *skills* blocklist for all three kinds (so a blocklisted skill `x` also suppresses agent `x`).
  Fix: per-kind blocklist entries. Effort **S**.
- **H4 ✔ `delete_source` blocklists every skill it unlinks** via the side effect in
  `uninstall_skill` (`src/skills.rs:246`, called at `:1103`). `rename_source` already has to undo
  this (`:2674–2680`). Fix: split `unlink_skill` (mechanical) from `uninstall_skill` (user intent
  → blocklist). Effort **XS–S**.
- **H5 ✔ `rename_source` is fragile and mis-handles Migrated sources.** (`src/skills.rs:2607`)
  (a) It uninstalls first, then on `fs::rename` failure rolls back via `old_path/skills/<n>`, wrong
  for root and nested skills (`:2687`). (b) For `Migrated` (`name = "agm_tools/<tool>"`), `new_path`
  is `source_dir/<new>` (`:2626`), moving it out of `agm_tools/` so TUI unlink recovery can no
  longer find it.
  - Fix (simpler, not just patched): rename the directory **first** (nothing to roll back on
    failure), then prune broken links and relink from a fresh scan; refuse renaming `Migrated`.
    Effort **S**.
- **H6 ✔ Tool config editor can write a config AGM can't load.** `src/tui/tool.rs:1874` validates
  only TOML syntax, then `fs::write`s the spliced file (`:1904`). `skills_dir = 1` passes, is
  written, and every later `agm` command fails to load config. Also: predictable temp path
  `temp_dir()/agm-<key>.toml` (`:1856`, collisions/symlink race), non-atomic write.
  - Fix: deserialize the section into `ToolConfig` before writing; write temp+rename; move
    `extract/replace_tool_section` (`:183–232`) into `config.rs`. Effort **S**.

#### 🟡 Medium
- **M1 ✅ `agm tool status` ignores `--config`.** `src/main.rs:755` calls `status::status()`;
  `src/status.rs:10` uses `Config::load()`. Repro: `--config /nonexistent/x.toml tool status`
  succeeds; `tool unlink` with the same flag errors. Effort **XS**.
- **M2 ✔ Editors with arguments fail, and the TUI hides the failure.** `src/editor.rs:16`
  `Command::new(editor)` with `EDITOR="code --wait"`; TUI callers `let _ = editor::open_files(…)`
  (`src/tui/tool.rs:1822`, `src/tui/source.rs:1106`). Fix: split program/args; surface errors in
  status + log. Effort **S**.
- **M3 ✔ Derived source names are never validated.** `repo_name_from_url("…/repo/")` → `""`
  (`src/skills.rs:508`), so `repo_path == source_dir` and `clone_or_pull` runs `git pull` there (or
  in an enclosing repo). Same gap for `add_local_copy` with a `..` path. (A scout rated this
  "Critical: deletes ~/.local/share"; not so — `remove_dir_all` only runs on a fresh clone path.)
  Fix: trim trailing `/`, always `validate_source_name`. Effort **XS**.
- **M4 ✔ URL handling gaps.** `is_url` (`:472`) misses `ssh://`, `git://`, `file://` (treated as
  local paths); `normalize_git_url` (`:518`) normalizes SSH but not HTTPS scheme, so the same repo
  via https vs ssh raises "belongs to a different repo". Effort **XS**.
- **M5 ✔ git can prompt on the TUI's terminal.** `git pull`/`clone` (`:586`, `:1008`) inherit stdin
  and don't set `GIT_TERMINAL_PROMPT=0`; a private HTTPS repo prompts for credentials mid-TUI (raw
  mode) or hangs the worker. Fix: `stdin(Stdio::null())` + env var. Effort **XS**.
- **M6 ✔ Filesystem I/O inside render.** `src/tui/tool.rs` `render_row` calls
  `compute_tool_status` and `linker::check_link` per row, per frame; `shell.rs:99` redraws every
  100 ms. Source screen already renders from a snapshot. Fix: compute statuses in `rebuild_rows`.
  Effort **M**.
- **M7 ◦ Bulk toggle mutates `install_status` in memory without rescanning**
  (`src/tui/source.rs:963` `execute_bulk_toggle`) — stale Conflict status; contradicts "State is
  derived". Fix: do the filesystem ops, then one `refresh()`. Effort **S**.
- **M8 ◦ Delete/rename/toggle/refresh allowed while a background git task runs** (`source.rs`).
  Gate on `background_task.is_running`. Effort **S**.
- **M9 ✔ `link_all`/`unlink_all` abort on the first error** with bare `?` and no path context
  (`src/main.rs:206, 236, 325, 333, 344, 388, 416`), leaving later tools untouched. Collect per-tool
  errors, continue, summarize; add `.with_context`. Effort **S**.
- **M10 ◦ `handle_blocked_link` claims "restored backup" after an ignored restore failure**
  (`src/tui/tool.rs:971`). Effort **XS**.
- **M11 ◦ Agent-only/command-only sources are rejected** ("No skills found… Clone removed",
  `src/skills.rs:1083`, `add_local_copy` `:893`). Confirm intent; if unintended, accept any Item.
  Effort **S**.
- **M12 ◦ `agm source add <local path>` ignores agents and commands** (`src/main.rs:473–498`) while
  the git branch handles agents. Effort **S**.

#### 🟢 Low
- **L1 ✔** `collapse_all` forgets `expanded_commands_sources` (`src/tui/source.rs:1773`). **XS**
- **L2 ◦** `select_all_in_group` counts only visible rows → 0 on a collapsed source (`:847`). **S**
- **L3 ◦** Footer advertises `d del` / `r rename` on rows where they are no-ops (`build_source_hints`). **XS**
- **L4 ◦** `linker::check_*_link` classifies a missing target reached via a relative/uncanonical
  path as `Wrong`, not `Broken`, so it is never repaired; `parent().unwrap()` (`src/linker.rs:46,72`). **S**
- **L5 ✔** Prompt backup `with_extension("<ts>.bak")` drops `.md` and can collide within one second
  (`src/main.rs:340`). **XS**
- **L6 ✔** Dead interactivity: `let yes = true` makes six `prompt_yes_no` calls unreachable
  (`src/main.rs:127,143`); `link_all` has an unused `_config_path` param. Delete, or add `--yes`. **XS**
- **L7 ◦** Column padding uses byte length (`source.rs:2507`); `TextInput` has no horizontal scroll. **S**
- **L8 ◦** Blocklist writes swallow errors (`skills.rs:412,424`); prune errors swallowed (`source.rs:181,361`). **XS**
- **L9 ◦** `init::run` loads config twice (`src/init.rs:27,47`); stale `#[allow(dead_code)]` on
  `paths::expand_path` (`src/paths.rs:24`). **XS**

### Simplicity & Clarity (the structural findings)

- **S1 ✔ Skill/Agent/Command are copy-pasted, not modeled.** `SkillInfo`/`AgentInfo`/`CommandInfo`
  identical (`src/skills.rs:21–44`); `install_agent`≡`install_command`, `uninstall_*`,
  `prune_broken_*`, `check_*_install_status`, `migrate_*_dir_quiet`; `scan_all_sources` repeats its
  mapping 3× (`:745/785/829`); in the TUI, `build_rows`, `execute_bulk_toggle`,
  `build_*_info_lines`, `expanded_*_sources` are triplicated. The glossary already names the
  abstraction (**Item**). Introduce `ItemKind { Skill, Agent, Command }` with `store_dir`,
  `link_name`, `is_dir` and one implementation per operation. This alone would have prevented H3,
  L1, M12, B2, B3. Effort **L** (do it incrementally, one operation per commit).
- **S2 ✔ The four Features are hand-unrolled.** `link_all`, `unlink_all` (4 near-identical blocks,
  `src/main.rs:382–422`), `status.rs:53–179` (4×30 lines), `tool.rs` `toggle_link` vs
  `execute_toggle_feature` (same ladder twice). A `Feature` enum + table (`key`, `is_dir`,
  `store_path(config)`) collapses each to a loop. Effort **M**.
- **S3 ✔ Orchestration lives in two front-ends.** "Inspect → migrate/backup → link" and
  "unlink → recover" exist in `main.rs` *and* `tui/tool.rs` (`handle_blocked_link`,
  `recover_after_unlink`, TOML splicing, `duplicate_name_count` in `source.rs:3089`). Every
  CLI/TUI divergence (B1, B2, C3, L5) comes from this. Move them into a domain module (e.g.
  `src/ops.rs`) that returns messages (ADR 0005 already requires quiet domain code); CLI prints,
  TUI logs. Effort **L**; highest leverage.
- **S4 ✔ Giant functions** (clippy `too_many_lines`, 25 over 100): `source.rs` `handle_key` 325,
  `render_list` 252, `build_category_info_lines` 215; `tool.rs` `render_row` 246,
  `recover_after_unlink` 200, `handle_key` 177; `status.rs` `status` 220; `main.rs` `link_all` 206.
  Most shrink naturally after S1–S3; split key handlers per mode, renderers per row variant.
- **S5 ◦ Source screen modal state is 7 loose fields** (`search_mode`, `add_mode`, `rename_mode`,
  `show_log` + `log_popup`, `info_popup`, `confirm_state`; `source.rs:80–117`) while `tool.rs`
  already uses a `PopupState` enum. Unify. Effort **S–M**.
- **S6 ✔ Shared TUI helpers duplicated**: `page_size`/`ensure_visible` (`source.rs:457`,
  `tool.rs:356`), `hint_key`/`hint_text` forwarding wrappers, terminal suspend/resume around the
  editor; `pending_editor_path` + `drain_pending_editor` exists only because one handler lacks the
  `terminal` param. Effort **S**.
- **S7 ✔ One "does this link point at X?" idiom, seven copies**: `read_link` + `canonicalize` +
  compare in `linker.rs:45,71` and `skills.rs:224,443,463,705,1190`. One `platform::points_to`
  fixes L4 and relative-link false Conflicts everywhere. Effort **S**.
- **S8 ✔** `skills.rs` (2,763 lines) mixes discovery, git, install, migration, blocklist, preload,
  rename; `mod tests` sits in the middle (`:1416–2441`) with production code after it. Move tests to
  the end now (XS); split into `skills/{scan,git,install,migrate,blocklist}.rs` with S1 (M).

### Security
- M5 (credential prompt on TUI tty), H6 (predictable `/tmp` file), M3 (unvalidated derived names).
- Dismissed: `git clone` without `--` — not reachable, since URLs must start with
  `http(s)://`/`git@` (`is_url`) or are rewritten to `https://github.com/…`.
- No secrets in repo; `auth` files are only listed, never read.

### Performance
- M6 is the only real hotspot. Scans are per-action, not per-frame, in the Source screen.

### Testing
- **T1** `tests/cli.rs` covers only `--help`/parsing. C1, C2, M1 each reproduced with a ~10-line
  temp-`$HOME` script — make those the first `assert_cmd` tests (set `HOME`, `XDG_*`). Effort **M**.
- **T2** No tests for: migration with unrecognized content / conflicts, `rename_source` failure
  path, `delete_source` blocklist side effect, per-kind blocklist, `poll` disconnect.
- **T3** Coverage not measured; `cargo llvm-cov` would give a baseline.
- Hygiene: `predicates` dev-dep unused; `dialoguer` `fuzzy-select` feature unused; 7 deprecated
  `Command::cargo_bin` calls and an unused `.assert()` result (`tests/cli.rs:71`);
  `assert_eq!(x, true)` ×3 (`src/tui/log.rs:154–163`). All **XS**.

---

## Prioritized Action Plan

1. **Quick wins (< 1 day)** — H2, H4, M1, M3, M4, M5, M2, H6 (schema check + atomic write +
   `tempfile`), L1, L5, L6, S8 (move tests), test/dependency hygiene. Each is XS–S and independent.
2. **Medium-term (1–5 days)** — T1 e2e tests pinning C1/C2/M1 **first**, then C1 (safe migration
   helper shared with B1), C2, H1, H3, H5 (rename-first), M6, M7–M9, S7.
3. **Long-term (> 5 days)** — S3 domain `ops` module shared by CLI and TUI (resolves C3, B1, B2 by
   construction), S1 `ItemKind`, S2 `Feature` table, S5 modal enum, then S4 splits fall out.
   Do S3 before S1/S2 so the unified code has a single home.

## Metrics
- Files analyzed: 22 Rust files + `Cargo.toml` · LOC: 14,346 (src 13,848; tests 498)
- Tests: 194 passed, 2 ignored (network/doctest) · Coverage: not measured
- `cargo clippy --all-targets`: 0 warnings in non-test code, 12 in tests
- `clippy::pedantic`: ~500 (mostly `uninlined_format_args`); 25 functions > 100 lines
- Largest files: `tui/source.rs` 3,211 · `tui/tool.rs` 3,084 · `skills.rs` 2,763
- Findings: 1 Critical, 8 High, 12 Medium, 9 Low, 8 structural
