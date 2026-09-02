# Sources, Skills, Agents, Commands

How AGM discovers, installs, and maintains **Item**s. Defined in `src/skills.rs`. Terms are from
[glossary.md](glossary.md).

## Central store layout

```
~/.local/share/agm/
  prompts/MASTER.md     real file — the shared Prompt
  skills/               links to Skill directories       ← linked into each Tool
  agents/               links to Agent .md files         ← linked into each Tool
  commands/             links to Command .md files       ← linked into each Tool
  .agm_uninstalled      the Blocklist (beside skills/)
  source/               real content
    <repo-name>/          SourceKind::Repo
    local/<name>/         SourceKind::Local
    agm_tools/<tool key>/ SourceKind::Migrated
```

Everything under `source/` is real content. Everything in `skills/`, `agents/`, `commands/` is a
**Link** into `source/`. The **Blocklist** lives in `skills_source`'s *parent*, not inside it,
so it is never mistaken for an installed **Skill**.

## Discovery

| **Item** | Marker | Scanner | Depth |
|---|---|---|---|
| **Skill** | a `SKILL.md` file in the directory | `scan_skills` | the source root itself, else recursive to depth 3 |
| **Agent** | any `*.md` file directly inside `<source>/agents/` | `scan_agents` | 1, no recursion |
| **Command** | any `*.md` file directly inside `<source>/commands/` | `scan_commands` | 1, no recursion |

`scan_skills` short-circuits: if `<path>/SKILL.md` exists the path *is* one **Skill** named after
its directory. Otherwise it walks subdirectories, and **stops descending as soon as it finds a
`SKILL.md`** — nested skills inside a skill are not discovered. **Agent** and **Command** names
are the file stem, sorted alphabetically.

**Source** kind is decided by directory name at the top of `source/`: `local` and `agm_tools`
are containers whose children are the sources; every other directory is a repo source, with its
URL read from `git remote`. Sources are sorted by name; a `Migrated` source is displayed as
`agm_tools/<tool key>`.

## Install status

Computed from the filesystem on every scan, never stored.

| Value | Condition |
|---|---|
| `NotInstalled` | nothing at the central path for that name |
| `Installed` | a link exists there and resolves to *this* **Source**'s file/directory |
| `Conflict` | something exists there but resolves elsewhere — another **Source** owns the name |

The name is the identity: `skills/<name>` for a **Skill**, `agents/<name>.md` and
`commands/<name>.md` for the others. Two sources shipping the same name cannot both be
installed.

## Install / uninstall

| Operation | Effect |
|---|---|
| `install_skill` | create the central directory if needed, drop the name from the **Blocklist**, then link `skills/<name>` → the source directory. Already correctly linked → no-op. Name taken by another source → error `Skill '<name>' already exists (installed from another source). Uninstall it first.` |
| `install_agent` / `install_command` | same, with a file link at `<name>.md`. Identity is checked with `same_file`, so a Windows hardlink counts as installed. |
| `uninstall_skill` | remove the link and **add the name to the Blocklist**. Never touches `source/`. |
| `uninstall_agent` / `uninstall_command` | remove the link. **No blocklist entry** — only skills are blocklisted. |

The **Blocklist** exists so that `agm source update`, which re-scans and installs newly appeared
**Skill**s, does not resurrect something the user deliberately removed. An explicit install
clears the entry.

## `prune_broken_*`

Each of `skills/`, `agents/`, `commands/` can be swept for links whose target no longer exists;
the count of removed links is returned. Pruning runs at the start of `agm tool link` (skills and
agents) and `agm source list`/`update` (all three).

## Add a source

| Input | Path taken | Lands in |
|---|---|---|
| `https://…`, `http://…`, `git@…` | `clone_or_pull` | `source/<repo name>/` |
| `user/repo` | rewritten to `https://github.com/user/repo`, then `clone_or_pull` | `source/repo/` |
| anything else | `add_local_copy` | `source/local/<dir name>/` |

- The repo name is the last URL segment with `.git` stripped. `-n/--name` overrides it and is
  validated by `validate_source_name` (rejects empty, `.`, `..`, and any `/` or `\`).
- `clone_or_pull` pulls instead of cloning when the target directory already exists, reporting
  `CloneAction::Clone` or `Pull`. Git stdout/stderr are **piped and forwarded as
  `CloneProgress::GitLine` events**, never inherited, so a TUI's display is never corrupted —
  pinned by `tests/source_ops.rs::clone_or_pull_routes_errors_through_callback_not_stdout`.
- `add_local_copy` **scans before copying** and errors if the directory contains no **Skill**;
  the original directory is left untouched.

## Update

`update_all_with_progress` deduplicates repos by git root, `git pull`s each, then re-syncs links:
newly appeared **Item**s are installed unless blocklisted, and broken links are pruned. Progress
arrives as `UpdateProgress::RepoStart` / `RepoComplete` / `AllDone { total, updated, new_skills,
new_agents, new_commands }`. Non-git sources (`local`, `agm_tools`) are skipped for pull but
still re-synced.

## Delete and rename

- `resolve_source_target` maps a `<target>` argument to exactly one **Source**: first an exact
  directory-name match, then a normalized git-URL match against repo origins (local sources are
  excluded from the URL pass). Ambiguity and no-match are both errors.
- `delete_source` removes every central **Link** owned by that **Source** — skills, agents,
  commands — and then deletes the source directory.
- `rename_source` validates the new name, refuses an existing target, renames the directory, and
  relinks only the **Item**s that were installed, reporting a `RenameReport` with per-kind
  relink counts plus `relink_failures` and `rollback_failures`.

URL comparison is normalized: trailing `/` and `.git` stripped, `git@host:user/repo` folded to
`host/user/repo`, lowercased.

## Migration

When `agm tool link` finds real content where a **Skill** link belongs, `migrate_tool_dir_quiet`
moves it into `source/agm_tools/<tool key>/` and links the migrated skills into the **Central
store**. Sibling functions do the same for `agents/` and `commands/`, skipping the tool's
`prompt_filename` so a `CLAUDE.md` is never adopted as an **Agent**. All three return
`(count, messages)` rather than printing.

## Preload chars

- **Skill**: sum of the character counts of the `name` and `description` values in the
  `SKILL.md` YAML frontmatter; `0` on any parse failure. Handles quoted values and block
  scalars.
- **Agent** / **Command**: character count of the whole file; `0` if unreadable.

## Machine-checked claims

`tests/source_ops.rs` pins: `resolve_by_directory_name`, `resolve_by_git_url`,
`resolve_no_match_errors`, `resolve_multi_url_match_errors`, `validate_names`,
`rename_relinks_installed_skill_only`, `rename_with_invalid_new_name_errors`,
`rename_target_exists_errors`, `clone_or_pull_routes_errors_through_callback_not_stdout`,
`clone_progress_variants_constructible`, and the **Preload chars** rules
(`preload_standard_keys`, `preload_quoted_values`, `preload_block_scalar`,
`preload_no_frontmatter`, `preload_missing_key`, `preload_missing_file`, `file_char_count_basic`,
`file_char_count_missing`). Discovery, install/uninstall, and blocklist behavior are covered by
the unit tests in `src/skills.rs`. Run with `cargo test`.
