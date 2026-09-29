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

## Pending verification

_None._

## Awaiting external

_None._

## Watching

_None._

## Done

| ID | Finding | Closed by |
|---|---|---|
