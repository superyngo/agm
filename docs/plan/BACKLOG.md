# Backlog
Status: In progress

The one living record of open work. Rows move to Done with the commit that closed them and are
never deleted. Evidence is file + symbol, never a line number. `Verified` is the date the row was
last checked against the tree — not when it was opened.

## Open

| ID | Opened | Verified | Pri | Finding | Evidence | Effort | Acceptance |
|---|---|---|---|---|---|---|---|

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
| B3 | `agm source list` omits commands | code-audit fixes (see CHANGELOG 2026-09-29) |
| B4 | `agm init` tells the user to run `agm link`, which does not exist | code-audit fixes (see CHANGELOG 2026-09-29) |
| B5 | Log popup ignores `⏎`/`␣` (callers drop `PopupAction::Close`) unlike every other scrollable popup | code-audit fixes (see CHANGELOG 2026-09-29) |
| B6 | Help panel lists `Ctrl+C` for the Source screen only; `shell::run` handles it on both | code-audit fixes (see CHANGELOG 2026-09-29) |
| B7 | `ToolConfig::resolve_path` is dead (`#[allow(dead_code)]`, test-only) and diverges from the runtime resolver | code-audit fixes (see CHANGELOG 2026-09-29) |
| B8 | Linker unit tests miss `Broken`, create-refusal (`Wrong`/`Blocked`), and remove-refusal branches | code-audit fixes (see CHANGELOG 2026-09-29) |
| B10 | The only test pinning "git output never reaches stdout" is `#[ignore]` (needs network) | code-audit fixes (see CHANGELOG 2026-09-29) |
| B11 | Code breaks two ADR rules: `#[cfg]` outside `platform.rs` (ADR 0003) and printing reachable from `tui/` (ADR 0005) | code-audit fixes (see CHANGELOG 2026-09-29) |
| B12 | Footer is fixed at 3 rows (1 content line), so the Source Manager's two-line branch never runs: background `⟳` progress and the selection count are never drawn | code-audit fixes (see CHANGELOG 2026-09-29) |
| B43 | S1 (remaining: TUI `build_rows` and `build_*_info_lines`; `skills.rs` agents/commands share `FileItemKind`, `scan_all_sources` and the Source Manager toggles are unified): Skill/Agent/Command are copy-pasted, not modeled | code-audit fixes (see CHANGELOG 2026-09-29) |
| B44 | S2 (remaining: `tool.rs` toggle ladders; CLI link/unlink and `status.rs` now loop over the feature table): The four Features are hand-unrolled | code-audit fixes (see CHANGELOG 2026-09-29) |
| B45 | S3 (remaining: TOML section splicing, `duplicate_name_count`, `handle_blocked_link` orchestration; migration dispatch is shared): Orchestration lives in two front-ends | code-audit fixes (see CHANGELOG 2026-09-29) |
| B46 | S4: giant functions (largest are now under 170 lines; 17 linear render/hint builders remain slightly over the 100-line lint) | code-audit fixes (see CHANGELOG 2026-09-29) |
