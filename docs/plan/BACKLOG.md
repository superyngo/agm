# Backlog
Status: In progress

The one living record of open work. Rows move to Done with the commit that closed them and are
never deleted. Evidence is file + symbol, never a line number. `Verified` is the date the row was
last checked against the tree — not when it was opened.

## Open

| ID | Opened | Verified | Pri | Finding | Evidence | Effort | Acceptance |
|---|---|---|---|---|---|---|---|
| B3 | 2026-09-29 | 2026-09-29 | P3 | `agm source list` omits commands | `src/main.rs` `source_list` | S | Each source block lists its commands with install status |
| B4 | 2026-09-29 | 2026-09-29 | P3 | `agm init` tells the user to run `agm link`, which does not exist | `src/init.rs` `run` | XS | Message names `agm tool link` |
| B5 | 2026-09-29 | 2026-09-29 | P3 | Log popup ignores `⏎`/`␣` (callers drop `PopupAction::Close`) unlike every other scrollable popup | `src/tui/tool.rs` `ToolApp::handle_popup_key`; `src/tui/source.rs` `App::handle_key` | XS | Decide: close on `⏎`/`␣` like other popups, or document as intentional; `KEYMAP.md` matches |
| B6 | 2026-09-29 | 2026-09-29 | P3 | Help panel lists `Ctrl+C` for the Source screen only; `shell::run` handles it on both | `src/tui/help.rs` `build_help_lines` | XS | Both screens' help list `Ctrl+C` |
| B7 | 2026-09-29 | 2026-09-29 | P3 | `ToolConfig::resolve_path` is dead (`#[allow(dead_code)]`, test-only) and diverges from the runtime resolver | `src/config.rs` `ToolConfig::resolve_path`; `src/tui/tool.rs` `ToolState::get_group_files` | S | One resolver, used at runtime and by the tests |
| B8 | 2026-09-29 | 2026-09-29 | P3 | Linker unit tests miss `Broken`, create-refusal (`Wrong`/`Blocked`), and remove-refusal branches | `src/linker.rs` `tests` | S | Each `LinkStatus` branch of `create_link`/`remove_link` has a test |
| B10 | 2026-09-29 | 2026-09-29 | P3 | The only test pinning "git output never reaches stdout" is `#[ignore]` (needs network) | `tests/source_ops.rs` `clone_or_pull_routes_errors_through_callback_not_stdout` | S | Invariant checked by a default `cargo test` run (offline fixture) |
| B11 | 2026-09-29 | 2026-09-29 | P2 | Code breaks two ADR rules: `#[cfg]` outside `platform.rs` (ADR 0003) and printing reachable from `tui/` (ADR 0005) | `src/linker.rs` `remove_link`, `remove_link_quiet`; `src/paths.rs` `contract_tilde`; `#[cfg(unix)]` on `src/skills.rs` tests `test_prune_broken_agents`, `test_prune_broken_commands`; `src/config.rs` `Config::resolved_link_path` (`eprintln!`) | M | Platform branches behind `platform::` helpers; `resolved_link_path` returns its warning to the caller; `rg '#\[cfg\((unix|windows|not)' src -g '!platform.rs'` finds nothing (tests use a `platform::` capability check instead) |
| B12 | 2026-09-29 | 2026-09-29 | P3 | Footer is fixed at 3 rows (1 content line), so the Source Manager's two-line branch never runs: background `⟳` progress and the selection count are never drawn | `src/tui/source.rs` `render_footer` (`inner.height >= 2`), `render` (`Constraint::Length(3)`) | S | Progress and selection count visible during a background update / active selection |
| B43 | 2026-09-29 | 2026-09-29 | P2 | S1 (remaining, TUI + `scan_all_sources`; agents/commands in `skills.rs` now share `FileItemKind`): Skill/Agent/Command are copy-pasted, not modeled | [code audit](../audit/2026-09-29-code-audit.md) §S1 | L | Fix in audit §S1 applied; regression test added |
| B44 | 2026-09-29 | 2026-09-29 | P2 | S2 (remaining: `status.rs`, `tool.rs` toggle ladders; CLI link/unlink now use `FEATURES`): The four Features are hand-unrolled | [code audit](../audit/2026-09-29-code-audit.md) §S2 | M | Fix in audit §S2 applied; regression test added |
| B45 | 2026-09-29 | 2026-09-29 | P2 | S3 (remaining: TOML section splicing, `duplicate_name_count`, `handle_blocked_link` orchestration; migration dispatch is shared): Orchestration lives in two front-ends | [code audit](../audit/2026-09-29-code-audit.md) §S3 | L | Fix in audit §S3 applied; regression test added |
| B46 | 2026-09-29 | 2026-09-29 | P2 | S4: Giant functions | [code audit](../audit/2026-09-29-code-audit.md) §S4 | S | Fix in audit §S4 applied; regression test added |

## Pending verification

_None._

## Awaiting external

_None._

## Watching

_None._

## Done

| ID | Finding | Closed by |
|---|---|---|
| B1 | `agm tool link` deletes a tool's non-empty real `agents/` directory without migrating or announcing it (auto-answered `yes`) | code-audit fixes (see CHANGELOG 2026-09-29) |
| B2 | `agm tool link` never links `commands`, while `unlink_all` and the Tool Manager do | code-audit fixes (see CHANGELOG 2026-09-29) |
| B13 | C1: Migration deletes unrecognized tool content and the tool's copy of conflicting items | code-audit fixes (see CHANGELOG 2026-09-29) |
| B14 | C2: `agm tool unlink` "copies back" symlinks, not content | code-audit fixes (see CHANGELOG 2026-09-29) |
| B15 | C3: CLI and TUI unlink have contradictory semantics | code-audit fixes (see CHANGELOG 2026-09-29) |
| B16 | H1: Source Manager keys UI state by `usize` group index across rescans | code-audit fixes (see CHANGELOG 2026-09-29) |
| B17 | H2: Background task can wedge forever | code-audit fixes (see CHANGELOG 2026-09-29) |
| B18 | H3: Uninstalled agents/commands come back on update | code-audit fixes (see CHANGELOG 2026-09-29) |
| B19 | H4: `delete_source` blocklists every skill it unlinks | code-audit fixes (see CHANGELOG 2026-09-29) |
| B20 | H5: `rename_source` is fragile and mis-handles Migrated sources | code-audit fixes (see CHANGELOG 2026-09-29) |
| B21 | H6: Tool config editor can write a config AGM can't load | code-audit fixes (see CHANGELOG 2026-09-29) |
| B22 | M1: `agm tool status` ignores `--config` | code-audit fixes (see CHANGELOG 2026-09-29) |
| B23 | M2: Editors with arguments fail, and the TUI hides the failure | code-audit fixes (see CHANGELOG 2026-09-29) |
| B24 | M3: Derived source names are never validated | code-audit fixes (see CHANGELOG 2026-09-29) |
| B25 | M4: URL handling gaps | code-audit fixes (see CHANGELOG 2026-09-29) |
| B26 | M5: git can prompt on the TUI's terminal | code-audit fixes (see CHANGELOG 2026-09-29) |
| B27 | M6: Filesystem I/O inside render | code-audit fixes (see CHANGELOG 2026-09-29) |
| B28 | M7: Bulk toggle mutates `install_status` in memory without rescanning | code-audit fixes (see CHANGELOG 2026-09-29) |
| B29 | M8: Delete/rename/toggle/refresh allowed while a background git task runs | code-audit fixes (see CHANGELOG 2026-09-29) |
| B30 | M9: `link_all`/`unlink_all` abort on the first error | code-audit fixes (see CHANGELOG 2026-09-29) |
| B31 | M10: `handle_blocked_link` claims "restored backup" after an ignored restore failure | code-audit fixes (see CHANGELOG 2026-09-29) |
| B32 | M11: Agent-only/command-only sources are rejected | code-audit fixes (see CHANGELOG 2026-09-29) |
| B33 | M12: `agm source add <local path>` ignores agents and commands | code-audit fixes (see CHANGELOG 2026-09-29) |
| B34 | L1: `collapse_all` forgets `expanded_commands_sources` | code-audit fixes (see CHANGELOG 2026-09-29) |
| B35 | L2: `select_all_in_group` counts only visible rows → 0 on a collapsed source | code-audit fixes (see CHANGELOG 2026-09-29) |
| B36 | L3: Footer advertises `d del` / `r rename` on rows where they are no-ops | code-audit fixes (see CHANGELOG 2026-09-29) |
| B37 | L4: `linker::check_*_link` classifies a missing target reached via a relative/uncanonical | code-audit fixes (see CHANGELOG 2026-09-29) |
| B38 | L5: Prompt backup `with_extension("<ts>.bak")` drops `.md` and can collide within one second | code-audit fixes (see CHANGELOG 2026-09-29) |
| B39 | L6: Dead interactivity: `let yes = true` makes six `prompt_yes_no` calls unreachable | code-audit fixes (see CHANGELOG 2026-09-29) |
| B41 | L8: Blocklist writes swallow errors | code-audit fixes (see CHANGELOG 2026-09-29) |
| B42 | L9: `init::run` loads config twice | code-audit fixes (see CHANGELOG 2026-09-29) |
| B47 | S5: Source screen modal state is 7 loose fields | code-audit fixes (see CHANGELOG 2026-09-29) |
| B48 | S6: Shared TUI helpers duplicated | code-audit fixes (see CHANGELOG 2026-09-29) |
| B49 | S7: One "does this link point at X?" idiom, seven copies | code-audit fixes (see CHANGELOG 2026-09-29) |
| B50 | S8: `skills.rs` mixed responsibilities with tests mid-file (tests moved to the end; module split not done) | code-audit fixes (see CHANGELOG 2026-09-29) |
| B40 | L7: column padding counted bytes and `TextInput` did not scroll horizontally | code-audit fixes (see CHANGELOG 2026-09-29) |
