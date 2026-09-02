# Unified Shell — Tab Switching Between Tool Manager and Source Manager — Implementation Plan
Status: Shipped (2026-09-02)

Paired spec: [2026-09-02-unified-shell-tab-switching.md](../spec/2026-09-02-unified-shell-tab-switching.md).

## Tasks

1. **Widen `ToolApp` visibility** (`src/tui/tool.rs`) — `new`, `ensure_visible`,
   `clear_expired_status`, `handle_key`, module-level `render` to `pub(crate)`; add
   `config()`, `is_modal()`, `should_quit()`, `drain_pending_editor()`.
2. **Tool Manager UI** — title chip `agm — [Tool] · Source`; append `Tab` / `source` hint to the
   footer's hint line.
3. **Remove `tool::run`** and trim now-unused imports (`anyhow::Result`; `crossterm::event::{self,
   Event, KeyEventKind}` — all three were used only inside the deleted function).
4. **Widen `App` visibility** (`src/tui/source.rs`) — struct, `ensure_visible`,
   `clear_expired_status`, `handle_key`, module-level `render` to `pub(crate)`; add `build()`
   (the old `run()`'s scan/prune/construct/kick-off-update prelude, minus the empty-sources
   early return — the existing empty-state message already covers zero sources), `config()`,
   `sync_config()`, `is_modal()`, `should_quit()`, `tick()` (the old `run()`'s inline
   `TaskEvent` drain, relocated verbatim).
5. **Source Manager UI** — title chip `agm — Tool · [Source]`; append `Tab` / `tool` hint to the
   footer's hint line.
6. **Remove `source::run`** and trim now-unused imports (`anyhow::Result`;
   `crossterm::event::{self, Event, KeyEventKind}` — `KeyCode`/`KeyModifiers` stay, still used
   throughout `handle_key`).
7. **Create `src/tui/shell.rs`** — `Tab` enum, `run(config_path, initial)`: owns panic
   hook/raw-mode/alt-screen once, lazily constructs the initial screen, loop: tick the Source
   screen's background work if built → `ensure_visible` → draw active screen → poll input →
   Ctrl+C quits → if `!is_modal()` and key is `Tab`/`BackTab`, switch screens (lazily building
   and `sync_config`-ing the target if needed) → else route the key to the active screen's
   `handle_key` (Tool additionally drains its pending-editor path after `handle_key`) → check
   `should_quit()`.
8. **Register the module** — `pub mod shell;` in `src/tui/mod.rs`, alphabetically ordered.
9. **Rewire `main.rs`** — bare invocation, `Commands::Tool { action: None }`, and
   `Commands::Source { action: None }` all call `tui::shell::run(cli.config.clone(), Tab::…)`;
   drop the now-unnecessary `mut` on the `Commands::Source` match arm's `config` binding.

## Verification performed

- `cargo build` — zero warnings.
- `cargo test` — 171 unit tests + 6 CLI integration tests + 17 source-ops integration tests, all
  passing (1 pre-existing `#[ignore]`d test, unrelated).
- Manual TUI smoke test (`tmux` capture-pane, scratch `--config`):
  - Bare `agm` opens the shell on the **Tool Manager** tab; title and footer show the tab chip
    and hint.
  - `Tab` switches to **Source Manager**; title flips; first visit fires the background update
    automatically (lazy `build()` + `do_update()`).
  - Entering search mode (`/`) then pressing `Tab` does **not** switch screens (`is_modal()`
    gate); `Esc` clears search, and `Tab` then switches normally.
  - Triggering an update (`u`), immediately switching to **Tool Manager**, waiting, switching
    back to **Source Manager** and opening the log (`o`) shows the update's `TaskEvent`s were
    drained the whole time (`tick()` runs regardless of active screen).
  - `agm tool --config …` opens the shell already on the **Tool Manager** tab; `?` opens
    Help/About, and `Tab` inside that popup cycles Help ↔ About without switching the shell's
    active screen (title chip unchanged).
  - `q` exits cleanly — no lingering process, terminal restored.

## Deviations from the spec

None.
