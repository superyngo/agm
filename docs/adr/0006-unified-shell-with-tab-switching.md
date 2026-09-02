# 0006 — One shell process hosting both TUIs, switched with Tab

Date: 2026-09-02

## Context

[0005](0005-ratatui-tui-as-primary-interface.md) made the **Tool Manager** (`agm tool`) and
**Source Manager** (`agm source`) two independent ratatui programs, each with its own panic
hook, raw-mode/alternate-screen setup, and event loop. Using both together meant quitting one
process and launching the other — no way to check a **Tool**'s **Link status** while looking at
the **Source Manager**'s tree, or vice versa, without leaving the terminal UI entirely.

Bare `agm` (no subcommand) printed help and exited 1, even though every other AGM state is
reachable from a TUI.

## Decision

Merge the two entry points into a single **Shell** process (`src/tui/shell.rs`) that owns the
terminal lifecycle once and hosts the **Tool Manager** and **Source Manager** as two screens,
switched with `Tab` / `Shift+Tab`. `agm tool` and `agm source` still exist and now mean "open the
**Shell**, focused on this screen"; bare `agm` opens the **Shell** on the **Tool Manager**.

Three structural choices follow from keeping [0005](0005-ratatui-tui-as-primary-interface.md)'s
rules intact rather than relaxing them:

- **Both screens can be alive at once, lazily.** A screen is constructed only the first time it
  is visited — `agm tool` alone still never touches the **Source Manager**'s startup scan or
  triggers a `git` update. Once built, a screen is never torn down for the rest of the process,
  so switching away and back preserves cursor position, expanded rows, and the log.
- **Background work ticks regardless of which screen is on top.** The **Source Manager**'s
  `TaskEvent` drain (previously inline in its own event loop) became `App::tick()`, called every
  shell iteration independent of `active`. A `git pull` started before switching to the **Tool
  Manager** keeps progressing and finishes correctly.
- **`Tab` is intercepted only when the active screen has no modal open.** Each screen exposes
  `is_modal()` (any popup, search/add/rename input, or confirmation); the shell checks it before
  treating `Tab`/`Shift+Tab` as a screen switch, otherwise the keypress goes to the screen's own
  `handle_key` — this is why the Help/About panel's own `Tab` cycle still works unchanged.

Config stays single-writer: the **Tool Manager** still owns `Config::save_to`. The **Source
Manager**, which never wrote config back even in the old per-process design (that write was
already dead code — it mutated a `&mut Config` the caller never persisted), now gets a fresh copy
via `App::sync_config` when the shell switches into it, so edits made on the **Tool Manager**
screen are visible immediately.

## Alternatives rejected

- **Re-enter the alternate screen per switch (trampoline).** Have each screen's `run()` return a
  "switch to the other screen" value and let a thin `main.rs` loop re-launch the other program.
  Cheaper to write, but every switch tears down and rebuilds the terminal (visible flicker),
  drops all in-memory state (cursor, expanded rows, log), and kills the **Source Manager**'s
  background thread mid-`git pull`. Rejected on user experience grounds.
- **Keep two processes, add a "launch the other TUI" keybinding that shells out.** Same
  state-loss problem as the trampoline, plus doubles process/terminal setup cost per switch.
- **Merge `tool.rs` and `source.rs` into one flat module/state machine.** Would touch far more
  code for no behavioral gain — the two screens' row models, key handling, and footers are
  already appropriately separate; only the entry point and terminal ownership were duplicated.

## Consequences

- `tool::run` and `source::run` no longer exist; `shell::run(config_path, initial_tab)` is the
  only TUI entry point, called from `main.rs` for bare `agm`, `agm tool`, and `agm source`.
- `ToolApp` and `App` (Source) both gained a narrow `pub(crate)` surface — `config()`,
  `is_modal()`, `should_quit()`, `ensure_visible()`, `handle_key()`, and (Source only) `build()`,
  `sync_config()`, `tick()` — so `shell.rs` can drive them without either screen knowing the
  other exists.
- [KEYMAP.md](../reference/KEYMAP.md) gained one truly global key (`Tab`/`Shift+Tab`) that did
  not exist before at the shell level; the Help/About panel's pre-existing `Tab` binding
  disambiguates by depending on `is_modal()`.
- [0005](0005-ratatui-tui-as-primary-interface.md)'s two structural rules (nothing blocks the UI
  thread, nothing prints) are unchanged and now apply shell-wide rather than per-process.
