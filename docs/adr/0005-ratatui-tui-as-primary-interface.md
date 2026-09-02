# 0005 — The TUI is the primary interface; git work runs off the UI thread

Date: 2026-04-01

## Context

The original interface was flag-driven commands plus blocking `dialoguer` prompts. Managing
dozens of skills across seven tools through them meant one prompt sequence per decision, no way
to see current state while deciding, and no batch operations. Worse, `update_all()` ran
synchronous `git pull` calls inline, freezing the display and leaving the terminal corrupted
afterward.

## Decision

Make two ratatui full-screen interfaces the primary way to use AGM — the **Tool Manager**
(`agm tool`) and the **Source Manager** (`agm source`) — and keep the non-interactive
subcommands (`link`, `unlink`, `status`, `source add/update/list/del/rename`) as the scriptable
surface.

Two structural rules follow:

- **Nothing blocks the UI thread.** Git work runs on a spawned thread and reports over an
  `mpsc` channel of `TaskEvent`s; the main loop drains it non-blockingly each tick. Git stdio is
  piped and forwarded as events, never inherited.
- **Nothing prints.** Domain code returns messages instead of printing. This is why
  `linker::create_link_quiet` / `remove_link_quiet` exist beside the printing versions, and why
  `skills`' migration functions return `(count, messages)`.

## Alternatives rejected

- **Keep improving the prompt-driven CLI.** No amount of prompt polish shows current state
  while the user decides, and batch selection over a tree does not fit a linear prompt.
- **A GUI or web UI.** Wrong medium: the tools being managed are terminal tools, and their
  config lives in the shell the user is already in.
- **Blocking git with a spinner.** Still freezes input, still corrupts the display on
  interleaved subprocess output, and still cannot be cancelled.

## Consequences

- The TUIs have no automated coverage, so [`../reference/tui.md`](../reference/tui.md) and
  [`../reference/KEYMAP.md`](../reference/KEYMAP.md) are hand-maintained; the in-app Help panel
  is generated from code and wins any disagreement.
- Any domain function a TUI may call must be print-free. A `println!` reachable from `tui/` is a
  display corruption bug, not a style issue.
- Because the filesystem is the source of truth ([0004](0004-skill-md-marker-and-derived-state.md)),
  every mutation ends in a re-scan. Selection therefore has to survive a re-scan, which is why
  it is snapshotted by `(category, source name, item name)` rather than by index.
- Two surfaces means two keymaps that must stay consistent; the shared keys are defined once in
  `tui/` and documented as *global* in `KEYMAP.md`.
