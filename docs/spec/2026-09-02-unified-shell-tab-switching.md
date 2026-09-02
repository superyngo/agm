# Unified Shell — Tab Switching Between Tool Manager and Source Manager — Design Spec
Status: Shipped (2026-09-02)

## Problem

The **Tool Manager** (`agm tool`) and **Source Manager** (`agm source`) were two independent
ratatui programs, each owning its own panic hook, raw-mode/alternate-screen setup, and event
loop (`src/tui/tool.rs::run`, `src/tui/source.rs::run`). Using both together meant quitting one
and launching the other — there was no way to check a **Tool**'s **Link status** while looking
at the **Source Manager**'s tree, or vice versa. Bare `agm` (no subcommand) printed help and
exited 1 instead of opening a TUI.

## Approach

Merge the two entry points into one **Shell** process (`src/tui/shell.rs`) that owns the
terminal lifecycle once and hosts both screens, switched with `Tab` / `Shift+Tab`. `agm tool`
and `agm source` become "open the shell, focused on this screen"; bare `agm` opens the shell on
the **Tool Manager**. See [ADR 0006](../adr/0006-unified-shell-with-tab-switching.md) for the
decision record and rejected alternatives (trampoline re-entry, shell-out to the other TUI).

## Screen ownership model

- **Lazy construction.** A screen (`ToolApp` / Source `App`) is built only the first time it is
  visited. `agm tool` alone never triggers the **Source Manager**'s startup scan or `git`
  background update, matching pre-merge behavior.
- **Persistent once built.** Neither screen is dropped for the life of the process. Switching
  away and back preserves cursor position, expanded rows, and the log — no re-entry flicker.
- **Background work ticks regardless of the active screen.** The **Source Manager**'s
  `TaskEvent` drain moved from its old inline event loop into `App::tick()`, called every shell
  iteration independent of which screen is `active`. A `git pull` started before switching to
  the **Tool Manager** keeps progressing and completes correctly.

## Tab interception

`Tab` / `Shift+Tab` switches screens **only** when the active screen reports `is_modal() ==
false`:

- Tool Manager: `help.is_some() || popup.is_some()` (the existing `PopupState` enum already
  covers every Tool Manager modal — Log, Info, PathEditor, ConfirmCreate, ConfirmToggleFeature).
- Source Manager: `help`, `log_popup`, `info_popup`, `search_mode`, `add_mode`, `rename_mode`, or
  `confirm_state` being active.

When modal, the keypress is routed to the screen's own `handle_key` as before — this is why the
Help/About panel's pre-existing `Tab` cycle (switching its own Help/About sub-tabs) is unchanged.

## Config synchronization

The **Tool Manager** remains the only screen that persists config via `Config::save_to`; the
**Source Manager** never wrote config back even before this change (`*config = app.config` in
the old `run()` mutated a `&mut Config` the caller never saved — dead code). When the shell
switches *into* the **Source Manager**, it calls `App::sync_config` with the **Tool Manager**'s
current in-memory config, so an edit made on the Tool tab (e.g. toggling a **Feature**) is
visible on the Source tab immediately, without needing to quit and relaunch.

## UI changes

- Title bar shows both screens with the active one bracketed: `agm — [Tool] · Source` /
  `agm — Tool · [Source]` (replaces the old per-process ` agm — Tool Manager ` /
  ` agm — Source Manager ` titles).
- Footer hint line ends with `Tab source` / `Tab tool` on both screens.
- CLI help / `docs/reference/cli.md`: bare `agm` now opens the shell instead of printing help and
  exiting 1.

## Scope

### In scope

- `src/tui/shell.rs`: new module owning the terminal lifecycle, event loop, and screen state
  (`Tab::Tool` / `Tab::Source`).
- `ToolApp` (`src/tui/tool.rs`) and `App` (`src/tui/source.rs`) widened to `pub(crate)` with a
  narrow driving surface: `config()`, `is_modal()`, `should_quit()`, `ensure_visible()`,
  `handle_key()`, module-level `render()`; Source additionally gains `build()`, `sync_config()`,
  `tick()`; Tool additionally gains `drain_pending_editor()`.
- Removal of `tool::run` and `source::run` (their terminal setup/event loop logic moved into
  `shell::run`).
- `main.rs`: bare-invocation, `Commands::Tool { action: None }`, and
  `Commands::Source { action: None }` all route through `tui::shell::run`.
- Tab chip in both screens' title bars; `Tab` hint appended to both footers.
- `docs/reference/cli.md`, `tui.md`, `KEYMAP.md`, `architecture.md`, `glossary.md`; `README.md`;
  this spec/plan pair; [ADR 0006](../adr/0006-unified-shell-with-tab-switching.md).

### Out of scope

- Any change to `agm tool link|unlink|status` or `agm source add|update|list|del|rename` —
  the non-interactive subcommands are untouched.
- Any change to either screen's internal row model, key handling, or footer hint content beyond
  the appended `Tab` hint — `tool.rs` and `source.rs` keep their existing, separate
  implementations.
- Reverse config sync (Source → Tool). Not needed because the Source Manager does not currently
  mutate config; documented as a follow-up trigger in ADR 0006 if that ever changes.
