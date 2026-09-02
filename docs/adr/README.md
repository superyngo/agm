# Architecture Decision Records

One file per decision that was expensive to reach and would be expensive to reverse. An ADR
records *why* and which alternatives were rejected; it is a historical record, never edited.
Current behavior lives in [`../reference/`](../reference/README.md).

| # | Decision | Status |
|---|---|---|
| [0001](0001-config-only-tool-registry.md) | Tools are registered in `config.toml`, not in code | Implemented (2026-02-14) |
| [0002](0002-links-not-copies.md) | One central store, reached by links rather than copies | Implemented (2026-02-14) |
| [0003](0003-platform-abstraction-for-windows-links.md) | Windows uses junctions and hardlinks behind a single `platform.rs` boundary | Implemented (2026-03-20) |
| [0004](0004-skill-md-marker-and-derived-state.md) | `SKILL.md` is the only marker; install state is derived from the filesystem | Implemented (2026-03-21) |
| [0005](0005-ratatui-tui-as-primary-interface.md) | The ratatui TUIs are the primary interface; git work runs off the UI thread | Implemented (2026-04-01) |

Partial supersessions: none.

These five records were written on 2026-09-02, after the fact, from the design specs in
[`../spec/`](../spec/README.md) and the shipped code. The dates above are the dates the
decisions landed, not the dates the records were written.
