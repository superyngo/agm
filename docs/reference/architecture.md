# Architecture

Module map and data flow of the `agm` binary. Terms are from [glossary.md](glossary.md).

## Shape

A single Rust binary plus a thin library (`src/lib.rs`) so integration tests can call internals.
All source is flat under `src/`, except the TUI, which is a module directory.

| Module | Responsibility |
|---|---|
| `main.rs` | clap command tree, command routing, the non-interactive `link_all` / `unlink_all` / `source_*` flows |
| `lib.rs` | re-exports the modules as a library for `tests/` |
| `config.rs` | `Config` / `AgmConfig` / `ToolConfig`, TOML load/save, defaults, path resolution per **Tool** |
| `paths.rs` | `expand_tilde`, `expand_path` (`$VAR`), `contract_tilde` |
| `linker.rs` | `LinkStatus` and the create/check/remove decision table, printing and quiet variants |
| `platform.rs` | the only place that knows Unix symlinks from Windows junctions/hardlinks; also the default editor |
| `skills.rs` | **Source** / **Skill** / **Agent** / **Command** discovery, install, update, delete, rename, migration, **Blocklist**, **Preload chars** |
| `status.rs` | the read-only `agm tool status` table |
| `init.rs` | `agm init`: default config plus **Central store** directories |
| `editor.rs` | editor resolution and process launch |
| `tui/` | the two ratatui surfaces and their shared widgets |

`tui/` splits into `mod.rs` (shared helpers), `tool.rs` and `source.rs` (the two managers),
`popup.rs` / `help.rs` / `log.rs` (overlays), `text_input.rs` (inline fields), `style.rs`
(style tokens, `NO_COLOR`), `background.rs` (off-thread git work).

## Layering

```
main.rs / tui/          command + interaction
  ↓
skills.rs  status.rs    domain operations
  ↓
linker.rs               link decisions
  ↓
platform.rs             OS primitives
  ↑
config.rs  paths.rs     configuration, used by every layer above
```

Nothing below `linker.rs` prints; `skills.rs` and the `*_quiet` linker functions return messages
so the TUIs can render them. `platform.rs` is the sole `#[cfg]` boundary — no other module
branches on the operating system.

## Data flow — `agm tool link`

1. `Config::load_from` reads the TOML.
2. Filter `config.tools` to those where `ToolConfig::is_installed()` (the **Config dir** exists).
3. Prune broken links in the **Central store**.
4. Per **Tool**, per **Feature**: `resolved_link_path` → inspect what is already there → migrate,
   back up, or delete as needed → `linker::create_link`.

See [cli.md](cli.md#agm-tool-link) for the full pre-handling table and
[linking.md](linking.md) for the decision table.

## Data flow — `agm source`

1. `skills::scan_all_sources` walks `source/`, classifying each directory as `Repo`, `Local`, or
   `Migrated`, and computing each **Item**'s **Install status** and **Preload chars** from disk.
2. The TUI renders that snapshot; actions mutate the filesystem and call `refresh()`, which
   re-scans. No state is cached between scans, so the filesystem is always the source of truth.
3. Git work runs on a background thread and reports progress over an `mpsc` channel — see
   [tui.md](tui.md#background-work).

## Invariants

- **Config drives everything.** Tools are data, not code. See
  [`../adr/0001-config-only-tool-registry.md`](../adr/0001-config-only-tool-registry.md).
- **Links, never copies.** The one exception is `agm tool unlink`, which deliberately copies
  content back so a tool keeps working after AGM steps out.
- **State is derived.** **Link status** and **Install status** are computed from the filesystem
  on every read; the only persisted state outside the config is the **Blocklist**.
- **Never destroy silently.** `linker` refuses to touch a real file or a wrongly-targeted link;
  destructive handling lives in the caller and is announced.

## Build and test

```sh
cargo build                # debug
cargo build --release      # → target/release/agm
cargo test                 # unit tests in src/*, integration tests in tests/
cargo test <name>          # single test
cargo test -- --nocapture  # show println! output
```

Unit tests live in each module behind `#[cfg(test)]` and use `tempfile`; `tests/cli.rs` and
`tests/source_ops.rs` are integration tests using `assert_cmd`. See
[releasing.md](releasing.md) for the release process.
