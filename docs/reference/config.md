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

## `[agm]` — the Central store

| Key | Type | Default | Meaning |
|---|---|---|---|
| `prompt_source` | string | `~/.local/share/agm/prompts/MASTER.md` | The shared **Prompt** file. |
| `skills_source` | string | `~/.local/share/agm/skills` | Central **Skill** directory. |
| `agents_source` | string | `~/.local/share/agm/agents` | Central **Agent** directory. |
| `commands_source` | string | `~/.local/share/agm/commands` | Central **Command** directory. |
| `source_dir` | string | `~/.local/share/agm/source` | Where **Source**s live. |
| `disabled` | array of string | `[]` | Globally disabled **Feature**s, from `prompt`, `skills`, `agents`, `commands`. |

The table accepts the legacy name `[central]` as a serde alias. `agents_source` and
`commands_source` have serde defaults, so a config predating them still loads.

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
- The four **Feature** fields are resolved as `config_dir.join(<value>)`.
- `resolve_path` treats a value as absolute when it contains `/` or `\`, starts with `~`, or has
  a drive letter; such values also get `$VAR` / `${VAR}` expansion, with unset variables left
  verbatim. Otherwise the value is relative to `config_dir`. This is what allows an absolute
  `auth` entry.
- A **Feature** whose resolved link path canonicalizes to the **Config dir** itself is refused
  with a warning, so a stray `"."` or `""` can never make AGM link over a tool's whole config
  directory.

## Pre-registered tools

`Config::default_config` ships seven tools. All seven use `skills`, `agents`, and `commands` as
their **Feature** directory names.

| Key | Name | `config_dir` | `prompt_filename` | `settings` | `auth` | `mcp` |
|---|---|---|---|---|---|---|
| `agy` | Antigravity CLI | `~/.gemini` | `GEMINI.md` | `antigravity-cli/settings.json` | `oauth_creds.json`, `accounts.json`, `google_accounts.json` | `config/mcp_config.json` |
| `claude` | Claude Code | `~/.claude` | `CLAUDE.md` | `~/.claude.json`, `settings.json`, `settings.local.json` | `.credentials.json` | `settings.json` |
| `codex` | Codex | `~/.codex` | `AGENTS.md` | `config.toml` | `auth.json` | `config.toml` |
| `copilot` | Copilot CLI | `~/.copilot` | `AGENTS.md` | `config.json` | `config.json` | `mcp-config.json` |
| `crush` | Crush | `~/.config/crush` | `AGENTS.md` | `crush.json` | `crush.json` | `crush.json` |
| `opencode` | OpenCode | `~/.config/opencode` | `AGENTS.md` | `opencode.json` | `~/.local/share/opencode/auth.json` | `opencode.json` |
| `pi` | Pi | `~/.pi/agent` | `AGENTS.md` | `settings.json` | `auth.json` | — |

`editor` defaults to `""`, which means "fall back to `$EDITOR`, then the platform default".

## `agm init`

Idempotent; every step reports `ok` or `skip`.

1. Write the default config if the file does not exist.
2. Create `skills_source`, `agents_source`, `commands_source`, `source_dir`.
3. Create `prompt_source`'s parent and seed the file with `# Shared AI Agent Prompt`.
4. Print each configured **Tool** as `installed` or `not found`.

## Machine-checked claims

`src/config.rs` unit tests cover the schema, defaults, `resolve_path`, `resolved_link_path`
collision refusal, and `is_installed`. `src/paths.rs` unit tests cover tilde and `$VAR`
expansion. Run with `cargo test`.
