# CLI

Current command surface of the `agm` binary, as defined in `src/main.rs`. Terms are from
[glossary.md](glossary.md).

## Global

```
agm [--config <PATH>] [-v|--version] [<command>]
```

| Flag | Effect |
|---|---|
| `-v`, `--version` | Print `agm <version>` and exit 0. The clap-generated `--version` is disabled; this flag is hand-rolled. |
| `--config <PATH>` | Override the config file path. Global — accepted on every subcommand. |

Invoking `agm` with no subcommand opens the unified **Shell** (see [tui.md](tui.md)) on the
**Tool Manager** tab. An unknown subcommand or a missing required argument still prints
**full help** (not clap's brief error) and exits **1**. All other parse errors use clap's
default handling.

## Commands

| Command | Interactive? | Does |
|---|---|---|
| `agm init` | no | Create the config file and the **Central store** directories. See [config.md](config.md#agm-init). |
| `agm config` | opens editor | Open the config file in the resolved editor. |
| `agm tool` | TUI | Open the **Shell** on the **Tool Manager** tab. See [tui.md](tui.md). |
| `agm tool link` | no | Link every installed **Tool**. |
| `agm tool unlink` | no | Unlink every installed **Tool** and copy content back. |
| `agm tool status` | no | Print the status table. |
| `agm source` | TUI | Open the **Shell** on the **Source Manager** tab. See [tui.md](tui.md). |
| `agm source add <source> [-n\|--name <name>] [--all]` | prompts | Add a **Source** from a URL, `user/repo` shorthand, or local path. |
| `agm source update` | no | `git pull` every repo **Source**, then re-sync links. |
| `agm source list` | no | List every **Source** with its **Item**s. |
| `agm source del <target>` | no | Delete a **Source** by directory name or repo URL. |
| `agm source rename <old> <new>` | no | Rename a **Source** directory and relink its installed **Item**s. |

There is no top-level `agm link` / `agm status`; those are `agm tool` subcommands. Older short
flags were removed and are asserted rejected by `tests/cli.rs::old_short_flags_rejected`.

## `agm tool link`

Order of work in `main.rs` `link_all`:

1. Prune broken **Skill** and **Agent** links from the **Central store**.
2. Print the globally disabled **Feature** list, if any.
3. For every installed **Tool**, in **Tool key** order: link `prompt`, `skills`, `agents`, then
   `commands` (the `FEATURES` table in `main.rs`). A failure in one **Tool** is reported and the
   remaining tools are still processed; the command exits non-zero with a summary at the end.

It runs non-interactively: it never asks, and it never destroys content it does not recognize.
Per-**Feature** pre-handling before the link is created:

| Existing state at the link path | `skills` / `agents` / `commands` | `prompt` |
|---|---|---|
| Correct link | left alone (`skip … already linked`) | left alone |
| Link to a different target | old link removed, relinked | old link removed, relinked |
| Real directory with content | recognised items **migrated** to `source/agm_tools/<tool key>/` (`agents/`, `commands/` subdirectories for those kinds); anything left behind (unrecognised files, the tool's own copy of an item already in the store) is kept by renaming the directory to `<dir>.agm-<YYYYMMDD_HHMMSS>.bak` and reported | n/a |
| Empty real directory | removed, then linked | n/a |
| Real file with content | n/a | renamed to `<name>.<YYYYMMDD_HHMMSS>.bak`, then linked |
| Real file that is blank | n/a | deleted, then linked |

A **Feature** is skipped when it is listed in `agm.disabled`, or when the **Tool**'s
corresponding field is empty, or when the resolved link path would collide with the **Config
dir** itself (a warning is printed and the field skipped, `Config::resolved_link_path`).

## `agm tool unlink`

For every installed **Tool** and each of the four **Feature**s: remove the link, and if removal
succeeded, **copy the Central store content back** into the tool's own path
(`skills::copy_dir_resolved`, which follows symlinks so the tool gets real files, or `fs::copy`
for the **Prompt**). A path that is not a link is left untouched with a warning. Unlink is
therefore not a pure inverse of link — the tool ends up with real copies, not the pre-AGM
content. The **Tool Manager** unlinks with the same semantics (`linker::detach_copy`).

## `agm source add`

1. `normalize_git_source` maps the argument: full HTTPS/SSH URL kept as-is; `user/repo` →
   `https://github.com/user/repo`; anything else treated as a local path.
2. URL → `clone_or_pull` into `source/<name>`; local path → `add_local_copy` into
   `source/local/<name>`. `-n/--name` overrides the derived directory name and is validated by
   `validate_source_name` (rejects empty, `.`, `..`, and names containing `/` or `\`).
3. Discovered **Skill**s are offered for install: one skill installs directly, several show a
   multi-select, `--all` installs everything without prompting. Any discovered **Agent**s in the
   source are installed automatically without prompting.

Git progress is streamed line by line through `CloneProgress::GitLine`; git output is piped, not
inherited, so it never corrupts the display.

## `agm source list`

Prunes broken links first, then prints one block per **Source**: its kind (`Repo` with URL,
`Local`, or `Migrated` from a tool), and each **Skill** and **Agent** with its **Install
status** (commands are omitted, tracked in [BACKLOG](../plan/BACKLOG.md) B3; preload chars are
not displayed).

## `agm tool status`

Prints a table of installed **Tool**s only (a **Tool** whose **Config dir** is absent is
skipped), then a **Central store** summary with installed counts. Per **Feature** row:

| Rendered | Meaning (`LinkStatus`) |
|---|---|
| `✓ linked → <path>` | `Linked` |
| `✗ missing → <target>` | `Missing` — nothing at the link path |
| `✗ broken` | `Broken` — link exists, target does not |
| `✗ wrong → <actual>` | `Wrong` — link points elsewhere |
| `✗ not linked → <path>` | `Blocked` — a real file or directory sits there |
| `disabled` | the **Feature** is in `agm.disabled` |

Paths are printed contracted to `~/…`.

## Editor resolution

`config.editor` (if non-empty) → `$EDITOR` → platform default (`vi` on Unix, `notepad` on
Windows). A non-zero editor exit becomes the error `Editor exited with error`.

## Environment

| Variable | Effect |
|---|---|
| `EDITOR` | Second choice in editor resolution, after `config.editor`. |
| `NO_COLOR` | When set to any value, including empty, color is stripped from TUI and CLI output; bold/italic are kept so the focus cursor stays visible. |

## Machine-checked claims

`tests/cli.rs` pins the command surface: `help_shows_tool_subcommands`,
`help_shows_source_subcommands`, `source_add_help_has_name_flag`, `old_short_flags_rejected`,
`source_del_parses`, `invalid_name_rejected_locally`.
