# AGM — Copilot Instructions

## Build, Test, and Run

```bash
cargo build                        # debug build
cargo build --release              # release binary → target/release/agm
cargo test                         # run all tests
cargo test <test_name>             # run a single test, e.g. cargo test test_scan_skills_single
cargo test -- --nocapture          # show println! output during tests
```

## Architecture

AGM is a single-binary Rust CLI. All source is flat under `src/`:

| File | Responsibility |
|---|---|
| `main.rs` | CLI definition (clap), command routing, interactive prompts |
| `config.rs` | `Config` / `ToolConfig` structs, TOML load/save at `~/.config/agm/config.toml` |
| `linker.rs` | `LinkStatus` enum, symlink create/check/remove logic |
| `skills.rs` | Skill discovery (`SKILL.md` convention), git clone/pull, add/remove |
| `paths.rs` | `expand_tilde` / `contract_tilde` utilities |
| `editor.rs` | Editor resolution: `config.editor` → `$EDITOR` → `vi` |
| `init.rs` | First-run setup: write default config + create central directories |
| `status.rs` | `status`, `list`, `check` commands — read-only display |

**Data flow for `agm link`:** `Config::load()` → iterate `config.tools` (skipping tools where `is_installed()` is false) → call `linker::create_link()` for each skills dir and prompt file.

**Central store layout** (defaults, overridable in config.toml):
```
~/.local/share/agm/
  prompts/MASTER.md      ← shared prompt; symlinked into each tool's config dir
  skills/                ← central skills dir; symlinked as each tool's skills dir
  source/                ← git-cloned skill repos live here
```

## Key Conventions

**Tool registration is config-only.** Tools are defined in `~/.config/agm/config.toml` under `[tools.<key>]`. A tool is considered "installed" if its `config_dir` directory exists on disk (`ToolConfig::is_installed()`). No code changes are needed to add a new tool.

**Skills are identified by `SKILL.md`.** A directory is a skill if it contains a `SKILL.md` file. `skills::scan_skills()` recurses up to depth 3 to find them. The central skills directory holds symlinks to the individual skill directories.

**Always use `paths::expand_tilde` / `contract_tilde`.** All paths from config are stored with `~/` prefix and must be expanded before use. Display paths back with `contract_tilde` for readability.

**`BTreeMap` for tools.** `Config.tools` is `BTreeMap<String, ToolConfig>`, so tools are always iterated in alphabetical order.

**Unix-only.** Symlink creation uses `std::os::unix::fs::symlink`; the tool is not designed for Windows.

**Tests use `tempfile`.** Unit tests in each module create isolated temp dirs with the `tempfile` crate. Integration tests use `assert_cmd`. Auth-file paths in `ToolConfig.auth` may be absolute (checked with `is_absolute()`) while all other per-tool file paths are relative to `config_dir`.
