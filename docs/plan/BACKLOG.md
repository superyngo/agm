# Backlog
Status: In progress

The one living record of open work. Rows move to Done with the commit that closed them and are
never deleted. Evidence is file + symbol, never a line number. `Verified` is the date the row was
last checked against the tree — not when it was opened.

## Open

| ID | Opened | Verified | Pri | Finding | Evidence | Effort | Acceptance |
|---|---|---|---|---|---|---|---|
| B1 | 2026-09-29 | 2026-09-29 | P1 | `agm tool link` deletes a tool's non-empty real `agents/` directory without migrating or announcing it (auto-answered `yes`) | `src/main.rs` `link_all` (agents branch, `fs::remove_dir_all`) | M | Existing agent files are migrated to `source/agm_tools/<tool key>/` (as the TUI's `migrate_agents_dir_quiet` does) or the link is refused; nothing is deleted unannounced |
| B2 | 2026-09-29 | 2026-09-29 | P2 | `agm tool link` never links `commands`, while `unlink_all` and the Tool Manager do | `src/main.rs` `link_all`, `unlink_all` | S | `agm tool link` links `commands` for every installed tool not disabling it |
| B3 | 2026-09-29 | 2026-09-29 | P3 | `agm source list` omits commands | `src/main.rs` `source_list` | S | Each source block lists its commands with install status |
| B4 | 2026-09-29 | 2026-09-29 | P3 | `agm init` tells the user to run `agm link`, which does not exist | `src/init.rs` `run` | XS | Message names `agm tool link` |
| B5 | 2026-09-29 | 2026-09-29 | P3 | Log popup ignores `⏎`/`␣` (callers drop `PopupAction::Close`) unlike every other scrollable popup | `src/tui/tool.rs` `ToolApp::handle_popup_key`; `src/tui/source.rs` `App::handle_key` | XS | Decide: close on `⏎`/`␣` like other popups, or document as intentional; `KEYMAP.md` matches |
| B6 | 2026-09-29 | 2026-09-29 | P3 | Help panel lists `Ctrl+C` for the Source screen only; `shell::run` handles it on both | `src/tui/help.rs` `build_help_lines` | XS | Both screens' help list `Ctrl+C` |
| B7 | 2026-09-29 | 2026-09-29 | P3 | `ToolConfig::resolve_path` is dead (`#[allow(dead_code)]`, test-only) and diverges from the runtime resolver | `src/config.rs` `ToolConfig::resolve_path`; `src/tui/tool.rs` `ToolState::get_group_files` | S | One resolver, used at runtime and by the tests |
| B8 | 2026-09-29 | 2026-09-29 | P3 | Linker unit tests miss `Broken`, create-refusal (`Wrong`/`Blocked`), and remove-refusal branches | `src/linker.rs` `tests` | S | Each `LinkStatus` branch of `create_link`/`remove_link` has a test |
| B10 | 2026-09-29 | 2026-09-29 | P3 | The only test pinning "git output never reaches stdout" is `#[ignore]` (needs network) | `tests/source_ops.rs` `clone_or_pull_routes_errors_through_callback_not_stdout` | S | Invariant checked by a default `cargo test` run (offline fixture) |
| B11 | 2026-09-29 | 2026-09-29 | P2 | Code breaks two ADR rules: `#[cfg]` outside `platform.rs` (ADR 0003) and printing reachable from `tui/` (ADR 0005) | `src/linker.rs` `remove_link`, `remove_link_quiet`; `src/paths.rs` `contract_tilde`; `#[cfg(unix)]` on `src/skills.rs` tests `test_prune_broken_agents`, `test_prune_broken_commands`; `src/config.rs` `Config::resolved_link_path` (`eprintln!`) | M | Platform branches behind `platform::` helpers; `resolved_link_path` returns its warning to the caller; `rg '#\[cfg\((unix|windows|not)' src -g '!platform.rs'` finds nothing (tests use a `platform::` capability check instead) |
| B12 | 2026-09-29 | 2026-09-29 | P3 | Footer is fixed at 3 rows (1 content line), so the Source Manager's two-line branch never runs: background `⟳` progress and the selection count are never drawn | `src/tui/source.rs` `render_footer` (`inner.height >= 2`), `render` (`Constraint::Length(3)`) | S | Progress and selection count visible during a background update / active selection |
| B13 | 2026-09-29 | 2026-09-29 | P1 | C1: Migration deletes unrecognized tool content and the tool's copy of conflicting items | [code audit](../audit/2026-09-29-code-audit.md) §C1 | M | Fix in audit §C1 applied; regression test added |
| B14 | 2026-09-29 | 2026-09-29 | P1 | C2: `agm tool unlink` "copies back" symlinks, not content | [code audit](../audit/2026-09-29-code-audit.md) §C2 | S | Fix in audit §C2 applied; regression test added |
| B15 | 2026-09-29 | 2026-09-29 | P1 | C3: CLI and TUI unlink have contradictory semantics | [code audit](../audit/2026-09-29-code-audit.md) §C3 | M | Fix in audit §C3 applied; regression test added |
| B16 | 2026-09-29 | 2026-09-29 | P1 | H1: Source Manager keys UI state by `usize` group index across rescans | [code audit](../audit/2026-09-29-code-audit.md) §H1 | M | Fix in audit §H1 applied; regression test added |
| B17 | 2026-09-29 | 2026-09-29 | P1 | H2: Background task can wedge forever | [code audit](../audit/2026-09-29-code-audit.md) §H2 | XS | Fix in audit §H2 applied; regression test added |
| B18 | 2026-09-29 | 2026-09-29 | P1 | H3: Uninstalled agents/commands come back on update | [code audit](../audit/2026-09-29-code-audit.md) §H3 | S | Fix in audit §H3 applied; regression test added |
| B19 | 2026-09-29 | 2026-09-29 | P1 | H4: `delete_source` blocklists every skill it unlinks | [code audit](../audit/2026-09-29-code-audit.md) §H4 | XS–S | Fix in audit §H4 applied; regression test added |
| B20 | 2026-09-29 | 2026-09-29 | P1 | H5: `rename_source` is fragile and mis-handles Migrated sources | [code audit](../audit/2026-09-29-code-audit.md) §H5 | S | Fix in audit §H5 applied; regression test added |
| B21 | 2026-09-29 | 2026-09-29 | P1 | H6: Tool config editor can write a config AGM can't load | [code audit](../audit/2026-09-29-code-audit.md) §H6 | S | Fix in audit §H6 applied; regression test added |
| B22 | 2026-09-29 | 2026-09-29 | P2 | M1: `agm tool status` ignores `--config` | [code audit](../audit/2026-09-29-code-audit.md) §M1 | XS | Fix in audit §M1 applied; regression test added |
| B23 | 2026-09-29 | 2026-09-29 | P2 | M2: Editors with arguments fail, and the TUI hides the failure | [code audit](../audit/2026-09-29-code-audit.md) §M2 | S | Fix in audit §M2 applied; regression test added |
| B24 | 2026-09-29 | 2026-09-29 | P2 | M3: Derived source names are never validated | [code audit](../audit/2026-09-29-code-audit.md) §M3 | XS | Fix in audit §M3 applied; regression test added |
| B25 | 2026-09-29 | 2026-09-29 | P2 | M4: URL handling gaps | [code audit](../audit/2026-09-29-code-audit.md) §M4 | XS | Fix in audit §M4 applied; regression test added |
| B26 | 2026-09-29 | 2026-09-29 | P2 | M5: git can prompt on the TUI's terminal | [code audit](../audit/2026-09-29-code-audit.md) §M5 | XS | Fix in audit §M5 applied; regression test added |
| B27 | 2026-09-29 | 2026-09-29 | P2 | M6: Filesystem I/O inside render | [code audit](../audit/2026-09-29-code-audit.md) §M6 | M | Fix in audit §M6 applied; regression test added |
| B28 | 2026-09-29 | 2026-09-29 | P2 | M7: Bulk toggle mutates `install_status` in memory without rescanning | [code audit](../audit/2026-09-29-code-audit.md) §M7 | S | Fix in audit §M7 applied; regression test added |
| B29 | 2026-09-29 | 2026-09-29 | P2 | M8: Delete/rename/toggle/refresh allowed while a background git task runs | [code audit](../audit/2026-09-29-code-audit.md) §M8 | S | Fix in audit §M8 applied; regression test added |
| B30 | 2026-09-29 | 2026-09-29 | P2 | M9: `link_all`/`unlink_all` abort on the first error | [code audit](../audit/2026-09-29-code-audit.md) §M9 | S | Fix in audit §M9 applied; regression test added |
| B31 | 2026-09-29 | 2026-09-29 | P2 | M10: `handle_blocked_link` claims "restored backup" after an ignored restore failure | [code audit](../audit/2026-09-29-code-audit.md) §M10 | XS | Fix in audit §M10 applied; regression test added |
| B32 | 2026-09-29 | 2026-09-29 | P2 | M11: Agent-only/command-only sources are rejected | [code audit](../audit/2026-09-29-code-audit.md) §M11 | S | Fix in audit §M11 applied; regression test added |
| B33 | 2026-09-29 | 2026-09-29 | P2 | M12: `agm source add <local path>` ignores agents and commands | [code audit](../audit/2026-09-29-code-audit.md) §M12 | S | Fix in audit §M12 applied; regression test added |
| B34 | 2026-09-29 | 2026-09-29 | P3 | L1: `collapse_all` forgets `expanded_commands_sources` | [code audit](../audit/2026-09-29-code-audit.md) §L1 | XS | Fix in audit §L1 applied; regression test added |
| B35 | 2026-09-29 | 2026-09-29 | P3 | L2: `select_all_in_group` counts only visible rows → 0 on a collapsed source | [code audit](../audit/2026-09-29-code-audit.md) §L2 | S | Fix in audit §L2 applied; regression test added |
| B36 | 2026-09-29 | 2026-09-29 | P3 | L3: Footer advertises `d del` / `r rename` on rows where they are no-ops | [code audit](../audit/2026-09-29-code-audit.md) §L3 | XS | Fix in audit §L3 applied; regression test added |
| B37 | 2026-09-29 | 2026-09-29 | P3 | L4: `linker::check_*_link` classifies a missing target reached via a relative/uncanonical | [code audit](../audit/2026-09-29-code-audit.md) §L4 | S | Fix in audit §L4 applied; regression test added |
| B38 | 2026-09-29 | 2026-09-29 | P3 | L5: Prompt backup `with_extension("<ts>.bak")` drops `.md` and can collide within one second | [code audit](../audit/2026-09-29-code-audit.md) §L5 | XS | Fix in audit §L5 applied; regression test added |
| B39 | 2026-09-29 | 2026-09-29 | P3 | L6: Dead interactivity: `let yes = true` makes six `prompt_yes_no` calls unreachable | [code audit](../audit/2026-09-29-code-audit.md) §L6 | XS | Fix in audit §L6 applied; regression test added |
| B40 | 2026-09-29 | 2026-09-29 | P3 | L7: Column padding uses byte length | [code audit](../audit/2026-09-29-code-audit.md) §L7 | S | Fix in audit §L7 applied; regression test added |
| B41 | 2026-09-29 | 2026-09-29 | P3 | L8: Blocklist writes swallow errors | [code audit](../audit/2026-09-29-code-audit.md) §L8 | XS | Fix in audit §L8 applied; regression test added |
| B42 | 2026-09-29 | 2026-09-29 | P3 | L9: `init::run` loads config twice | [code audit](../audit/2026-09-29-code-audit.md) §L9 | XS | Fix in audit §L9 applied; regression test added |
| B43 | 2026-09-29 | 2026-09-29 | P2 | S1: Skill/Agent/Command are copy-pasted, not modeled | [code audit](../audit/2026-09-29-code-audit.md) §S1 | L | Fix in audit §S1 applied; regression test added |
| B44 | 2026-09-29 | 2026-09-29 | P2 | S2: The four Features are hand-unrolled | [code audit](../audit/2026-09-29-code-audit.md) §S2 | M | Fix in audit §S2 applied; regression test added |
| B45 | 2026-09-29 | 2026-09-29 | P2 | S3: Orchestration lives in two front-ends | [code audit](../audit/2026-09-29-code-audit.md) §S3 | L | Fix in audit §S3 applied; regression test added |
| B46 | 2026-09-29 | 2026-09-29 | P2 | S4: Giant functions | [code audit](../audit/2026-09-29-code-audit.md) §S4 | S | Fix in audit §S4 applied; regression test added |
| B47 | 2026-09-29 | 2026-09-29 | P2 | S5: Source screen modal state is 7 loose fields | [code audit](../audit/2026-09-29-code-audit.md) §S5 | S–M | Fix in audit §S5 applied; regression test added |
| B48 | 2026-09-29 | 2026-09-29 | P2 | S6: Shared TUI helpers duplicated | [code audit](../audit/2026-09-29-code-audit.md) §S6 | S | Fix in audit §S6 applied; regression test added |
| B49 | 2026-09-29 | 2026-09-29 | P2 | S7: One "does this link point at X?" idiom, seven copies | [code audit](../audit/2026-09-29-code-audit.md) §S7 | S | Fix in audit §S7 applied; regression test added |
| B50 | 2026-09-29 | 2026-09-29 | P2 | S8:  | [code audit](../audit/2026-09-29-code-audit.md) §S8 | S | Fix in audit §S8 applied; regression test added |

## Pending verification

_None._

## Awaiting external

_None._

## Watching

_None._

## Done

| ID | Finding | Closed by |
|---|---|---|
