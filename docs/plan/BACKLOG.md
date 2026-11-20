# Backlog
Status: In progress

The one living record of open work. Rows move to Done with the commit that closed them and are
never deleted. Evidence is file + symbol, never a line number. `Verified` is the date the row was
last checked against the tree — not when it was opened.

## Open

| ID | Opened | Verified | Pri | Finding | Evidence | Effort | Acceptance |
|---|---|---|---|---|---|---|---|
| B51 | 2026-11-17 | 2026-11-20 | P1 | v0.17.0 Windows release jobs (`x86_64`/`i686-pc-windows-msvc`) fail: the nested `agy` feature link `~/.gemini/config/skills` is not created although `agm tool link` exits 0. Diagnosis: [debug note](../debug/2026-11-17-windows-nested-feature-link.md) | `tests/e2e.rs` `link_creates_missing_parent_of_nested_feature_dir`; `src/main.rs` `link_tool`; `src/config.rs` `ToolConfig::resolve_link`; `src/linker.rs` `do_link` | M | Both Windows jobs of `.github/workflows/release.yml` pass `cargo test --test e2e` on a new tag |

## Pending verification

_None._

## Awaiting external

_None._

## Watching

| Item | Why not now | Trigger | Re-read |
|---|---|---|---|
| B52: `link_migrates_existing_agents_instead_of_deleting` failed only on `i686-pc-windows-msvc` (`agm.exe init` → `os error 2`) in the v0.17.0 run; `tests/e2e.rs` | Uninvestigated; may be CI flakiness, and B51 must be fixed first ([debug note](../debug/2026-11-17-windows-nested-feature-link.md) §Related) | Fails again on the i686 job after B51 lands | 2026-11-20 |
| B53: `src/skills.rs` module split not done (residual of B50 / audit §S8) | Tests already moved to the end; a split is churn without a feature driving it | Next feature that adds a responsibility to `src/skills.rs` | 2026-11-20 |
| B54: blocked-link orchestration still lives in the Tool Manager only (`src/tui/tool.rs` `ToolApp::handle_blocked_link`) — residual of B45 / audit §S3 | Migration dispatch and TOML splicing are shared; the CLI path works | Next change to blocked-link handling in either front-end | 2026-11-20 |
| B55: linear render/hint builders slightly over the 100-line lint — residual of B46 / audit §S4 | Linear builders; splitting them adds indirection | `clippy::too_many_lines` enabled, or a builder grows further | 2026-11-20 |

## Done

| ID | Finding | Closed by |
|---|---|---|
| B1 | `agm tool link` deletes a tool's non-empty real `agents/` directory without migrating or announcing it (auto-answered `yes`) | `a0904d7` |
| B2 | `agm tool link` never links `commands`, while `unlink_all` and the Tool Manager do | `0bbacee` |
| B13 | C1: Migration deletes unrecognized tool content and the tool's copy of conflicting items | `a0904d7` |
| B14 | C2: `agm tool unlink` "copies back" symlinks, not content | `a0904d7` |
| B15 | C3: CLI and TUI unlink have contradictory semantics | `db19d2e` |
| B16 | H1: Source Manager keys UI state by `usize` group index across rescans | `4431017` |
| B17 | H2: Background task can wedge forever | `092d1f5` |
| B18 | H3: Uninstalled agents/commands come back on update | `3681ba2` |
| B19 | H4: `delete_source` blocklists every skill it unlinks | `3681ba2` |
| B20 | H5: `rename_source` is fragile and mis-handles Migrated sources | `3681ba2` |
| B21 | H6: Tool config editor can write a config AGM can't load | `092d1f5` |
| B22 | M1: `agm tool status` ignores `--config` | `092d1f5` |
| B23 | M2: Editors with arguments fail, and the TUI hides the failure | `092d1f5` |
| B24 | M3: Derived source names are never validated | `092d1f5` |
| B25 | M4: URL handling gaps | `092d1f5` |
| B26 | M5: git can prompt on the TUI's terminal | `092d1f5` |
| B27 | M6: Filesystem I/O inside render | `4431017` |
| B28 | M7: Bulk toggle mutates `install_status` in memory without rescanning | `0bbacee` |
| B29 | M8: Delete/rename/toggle/refresh allowed while a background git task runs | `0bbacee` |
| B30 | M9: `link_all`/`unlink_all` abort on the first error | `0bbacee` |
| B31 | M10: `handle_blocked_link` claims "restored backup" after an ignored restore failure | `a2ffc23` |
| B32 | M11: Agent-only/command-only sources are rejected | `c052d23` |
| B33 | M12: `agm source add <local path>` ignores agents and commands | `c052d23` |
| B34 | L1: `collapse_all` forgets `expanded_commands_sources` | `092d1f5` |
| B35 | L2: `select_all_in_group` counts only visible rows → 0 on a collapsed source | `a2ffc23` |
| B36 | L3: Footer advertises `d del` / `r rename` on rows where they are no-ops | `a2ffc23` |
| B37 | L4: `linker::check_*_link` classifies a missing target reached via a relative/uncanonical | `c052d23` |
| B38 | L5: Prompt backup `with_extension("<ts>.bak")` drops `.md` and can collide within one second | `092d1f5` |
| B39 | L6: Dead interactivity: `let yes = true` makes six `prompt_yes_no` calls unreachable | `0bbacee` |
| B41 | L8: Blocklist writes swallow errors | `3681ba2` |
| B42 | L9: `init::run` loads config twice | `d4d0717` |
| B47 | S5: Source screen modal state is 7 loose fields | `c8f197a` |
| B48 | S6: Shared TUI helpers duplicated | `d4d0717` |
| B49 | S7: One "does this link point at X?" idiom, seven copies | `72bb39f` |
| B50 | S8: `skills.rs` mixed responsibilities with tests mid-file (tests moved to the end; module split → Watching B53) | `092d1f5` |
| B40 | L7: column padding counted bytes and `TextInput` did not scroll horizontally | `ef06f49` |
| B3 | `agm source list` omits commands | `b3b5a57` |
| B4 | `agm init` tells the user to run `agm link`, which does not exist | `d4d0717` |
| B5 | Log popup ignores `⏎`/`␣` (callers drop `PopupAction::Close`) unlike every other scrollable popup | `b3b5a57` |
| B6 | Help panel lists `Ctrl+C` for the Source screen only; `shell::run` handles it on both | `b3b5a57` |
| B7 | `ToolConfig::resolve_path` is dead (`#[allow(dead_code)]`, test-only) and diverges from the runtime resolver | `1f91fe9` |
| B8 | Linker unit tests miss `Broken`, create-refusal (`Wrong`/`Blocked`), and remove-refusal branches | `7bbeb7b` |
| B10 | The only test pinning "git output never reaches stdout" is `#[ignore]` (needs network) | `7bbeb7b` |
| B11 | Code breaks two ADR rules: `#[cfg]` outside `platform.rs` (ADR 0003) and printing reachable from `tui/` (ADR 0005) | `7bbeb7b` |
| B12 | Footer is fixed at 3 rows (1 content line), so the Source Manager's two-line branch never runs: background `⟳` progress and the selection count are never drawn | `0d8a2dd` |
| B43 | S1: Skill/Agent/Command are copy-pasted, not modeled | `1783814` |
| B44 | S2: The four Features are hand-unrolled | `441fe9d` |
| B45 | S3: Orchestration lives in two front-ends (residual → Watching B54) | `1190538` |
| B46 | S4: giant functions (residual → Watching B55) | `42cbda1` |
