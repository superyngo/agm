# Configuration

Schema and path semantics of `config.toml`, as defined in `src/config.rs` and `src/paths.rs`.
Terms are from [glossary.md](glossary.md).

## Location

| | Path |
|---|---|
| Config file | `~/.config/agm/config.toml` (`Config::config_path`) |
| Override | `agm --config <PATH>` on any command |

Loading a missing file is a hard error: `Config not found at <path>. Run `agm init` first.`
Malformed TOML surfaces the `toml` parse error. Saving writes
`toml::to_string_pretty`, creating parent directories as needed — **comments and key order in a
hand-edited file are not preserved** when AGM saves (the TUI saves on config edits).

## Top-level schema

| Key | Type | Default | Meaning |
|---|---|---|---|
| `editor` | string | `""` | Command or path for editor resolution (falls back to `$EDITOR`, then platform default). |
| `agm` | table | see below | Central store configuration (accepts legacy alias `[central]`). |
| `tools` | table of tables | see below | Per-tool configurations keyed by **Tool key**. |

## `[agm]` — the Central store

| Key | Type | Default | Meaning |
|---|---|---|---|
| `prompt_source` | string | `~/.local/share/agm/prompts/MASTER.md` | The shared **Prompt** file. |
| `skills_source` | string | `~/.local/share/agm/skills` | Central **Skill** directory. |
| `agents_source` | string | `~/.local/share/agm/agents` | Central **Agent** directory. |
| `commands_source` | string | `~/.local/share/agm/commands` | Central **Command** directory. |
| `source_dir` | string | `~/.local/share/agm/source` | Where **Source**s live. |
| `disabled` | array of string | `[]` | Globally disabled **Feature**s, from `prompt`, `skills`, `agents`, `commands`. |

The table accepts the legacy name `[central]` as a serde alias. `agents_source`,
`commands_source`, and `disabled` have serde defaults, so a config predating them still loads.

## `[tools.<key>]`

The `<key>` is the **Tool key**. `Config.tools` is a `BTreeMap`, so tools are always processed
and displayed in alphabetical key order.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | required | Human-readable name shown in output. |
| `config_dir` | string | required | The **Config dir**. Its existence defines *installed*. |
| `prompt_filename` | string | `""` | Filename inside `config_dir` that the **Prompt** is linked to. Empty = feature not configured. |
| `skills_dir` | string | `""` | Directory inside `config_dir` linked to the central skills dir. |
| `agents_dir` | string | `""` | Directory inside `config_dir` linked to the central agents dir. |
| `commands_dir` | string | `""` | Directory inside `config_dir` linked to the central commands dir. |
| `settings` | array of string | `[]` | Settings files offered for editing in the **Tool Manager**. |
| `auth` | array of string | `[]` | Credential files offered for editing. Entries may be absolute (e.g. `~/.local/share/opencode/auth.json`). |
| `mcp` | array of string | `[]` | MCP config files offered for editing. |

**Registration is config-only**: adding a tool means adding a `[tools.<key>]` table. No code
change, no recompile — see [`../adr/0001-config-only-tool-registry.md`](../adr/0001-config-only-tool-registry.md).

### Path semantics

- Every path string is stored with a `~/` prefix where applicable and expanded at use time
  (`paths::expand_tilde`, handling `~`, `~/`, and `~\`). Paths are contracted back to `~/…` for
  display (`paths::contract_tilde`).
- `~` is `paths::home_dir()`: the `AGM_HOME` environment variable when set and non-empty,
  otherwise the platform home (`dirs::home_dir`). `AGM_HOME` exists so tests can isolate a
  throwaway home on Windows, where `dirs` ignores `$HOME`; normal use leaves it unset.
- `ToolConfig::resolve_path` is the one resolver, used for the four **Feature** fields and for
  the `settings`, `auth`, `mcp` file groups (`ToolState::get_group_files` calls it). Absolute,
  `~` and `$VAR` entries are expanded; everything else, including `sub/dir/file`, is joined to
  `config_dir`.
- A **Feature** whose resolved link path canonicalizes to the **Config dir** itself is refused
  with a warning, so a stray `"."` or `""` can never make AGM link over a tool's whole config
  directory. A relative **Feature** value containing `..` is refused the same way.
- Missing parent directories of a **Feature** link path (e.g. `~/.gemini/config/` for
  `config/skills`) are created when the link is made.
- `Config::save_to` writes atomically (temp file in the same directory, then rename).

## Pre-registered tools

`Config::default_config` ships eight tools. All use `skills`, `agents`, and `commands` as their
**Feature** directory names, except `agy`: Antigravity reads personal skills and agents from
`~/.gemini/config/` (shared by the Antigravity App and CLI) and has no commands concept, so it uses
`config/skills`, `config/agents` and an empty `commands_dir`.

Defaults only seed a fresh config. To move an existing install to the new `agy` layout, run
`agm tool unlink`, edit `[tools.agy]`, then `agm tool link`; otherwise the old
`~/.gemini/skills` link stays and the CLI discovers the same skills twice.

| Key | Name | `config_dir` | `prompt_filename` | `settings` | `auth` | `mcp` |
|---|---|---|---|---|---|---|
| `agy` | Antigravity CLI | `~/.gemini` | `AGENTS.md` | `antigravity-cli/settings.json` | `oauth_creds.json`, `accounts.json`, `google_accounts.json` | `config/mcp_config.json` |
| `claude` | Claude Code | `~/.claude` | `CLAUDE.md` | `~/.claude.json`, `settings.json`, `settings.local.json` | `.credentials.json` | `settings.json` |
| `codex` | Codex | `~/.codex` | `AGENTS.md` | `config.toml` | `auth.json` | `config.toml` |
| `copilot` | Copilot CLI | `~/.copilot` | `AGENTS.md` | `config.json` | `config.json` | `mcp-config.json` |
| `crush` | Crush | `~/.config/crush` | `AGENTS.md` | `crush.json` | `crush.json` | `crush.json` |
| `omp` | OMP | `~/.omp/agent` | `SYSTEM.md` | `config.yml` | — | `mcp.json` |
| `opencode` | OpenCode | `~/.config/opencode` | `AGENTS.md` | `opencode.json` | `~/.local/share/opencode/auth.json` | `opencode.json` |
| `pi` | Pi | `~/.pi/agent` | `AGENTS.md` | `settings.json` | `auth.json` | — |

## `agm init`

Idempotent; every step reports `ok` or `skip`.

1. Write the default config if the file does not exist.
2. Create `skills_source`, `agents_source`, `commands_source`, `source_dir`.
3. Create `prompt_source`'s parent and seed the file with `# Shared AI Agent Prompt`.
4. Print each configured **Tool** as `installed` or `not found`.

## Machine-checked claims

`src/config.rs` unit tests cover the schema, defaults, `resolve_path`, `resolved_link_path`
collision and `..` refusal, section editing, atomic save, and `is_installed`. `src/paths.rs` unit tests cover tilde and `$VAR`
expansion. Run with `cargo test`.
