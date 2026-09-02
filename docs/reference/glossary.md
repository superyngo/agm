# Glossary

Canonical vocabulary for AGM. Code identifiers, CLI strings, TUI labels, commit messages, and
every other document use these terms. Introducing a new term means adding its entry here in the
same commit.

**Tool**:
An AI coding agent CLI that AGM manages — Claude Code, Codex, Copilot CLI, Crush, Antigravity
CLI, OpenCode, Pi. A **Tool** exists because it has a `[tools.<key>]` entry in the config; it is
*installed* when its **Config dir** exists on disk. Represented by `ToolConfig`.
_Avoid_: Client, agent (an **Agent** is a different thing), CLI.

**Tool key**:
The short lowercase identifier a **Tool** is addressed by — `claude`, `codex`, `copilot`,
`crush`, `agy`, `opencode`, `pi`. It is the `BTreeMap` key under `[tools.<key>]`, distinct from
the human-readable `name` field ("Claude Code").
_Avoid_: Tool id, slug.

**Config dir**:
A **Tool**'s own configuration directory (`~/.claude`, `~/.codex`, …), given by
`ToolConfig.config_dir`. Every other per-tool path except an absolute `auth` entry is resolved
relative to it. Its existence is the definition of *installed*.
_Avoid_: Tool home, tool root.

**Central store**:
The AGM-owned directory tree that holds the single copy of everything shared across **Tool**s —
by default `~/.local/share/agm/` with `prompts/MASTER.md`, `skills/`, `agents/`, `commands/`,
and `source/`. Configured under `[agm]`.
_Avoid_: Central dir, agm dir, hub.

**Prompt**:
The shared instruction file (`~/.local/share/agm/prompts/MASTER.md` by default) that is linked
into each **Tool**'s **Config dir** under that tool's own `prompt_filename` (`CLAUDE.md`,
`AGENTS.md`, `GEMINI.md`).
_Avoid_: Master prompt, system prompt, memory file.

**Feature**:
One of the four linkable categories: `prompt`, `skills`, `agents`, `commands`
(`AgmConfig::TOGGLEABLE_FEATURES`). A **Feature** can be globally disabled via `agm.disabled`,
and is skipped per **Tool** when that tool's corresponding field is empty.
_Avoid_: Category, kind, feature toggle (that is the *mechanism*, not the thing).

**Link**:
The filesystem indirection AGM creates from a **Tool**'s **Config dir** to the **Central
store** — a symlink on Unix; on Windows a junction for directories and a hardlink for files.
AGM only ever creates and removes **Link**s; it never copies content into a **Tool**.
_Avoid_: Symlink (platform-specific), shortcut, alias.

**Link status**:
The result of comparing a **Link**'s actual state against its expected target: `Linked`,
`Wrong`, `Blocked`, `Missing`, or `Broken` (`LinkStatus`). See
[linking.md](linking.md) for each meaning.
_Avoid_: Link state, health.

**Source**:
One directory under the **Central store**'s `source/` holding upstream material: a cloned git
repo, a copied local directory (under `source/local/`), or content migrated out of a **Tool**
(under `source/agm_tools/`). Represented by `SourceGroup` with a `SourceKind` of `Repo`,
`Local`, or `Migrated`.
_Avoid_: Repo (only one of three kinds), package, registry.

**Item**:
The umbrella term for one installable unit inside a **Source** — a **Skill**, an **Agent**, or a
**Command**. Used when a rule applies to all three.
_Avoid_: Entry, artifact, asset.

**Skill**:
A directory containing a `SKILL.md` file. That file's presence is the whole definition;
discovery recurses up to depth 3 looking for it (`scan_skills`). Installing a **Skill** links
its directory into the **Central store**'s `skills/`.
_Avoid_: Plugin, module, capability.

**Agent**:
A single `.md` file inside a **Source**'s `agents/` directory. Installing an **Agent** links
that one file into the **Central store**'s `agents/`.
_Avoid_: Subagent, persona, role.

**Command**:
A single `.md` file inside a **Source**'s `commands/` directory, installed by linking that file
into the **Central store**'s `commands/`. Structurally identical to an **Agent**, different
directory and purpose.
_Avoid_: Slash command, prompt template.

**Install status**:
Whether an **Item** is currently linked into the **Central store**: `Installed`, `NotInstalled`,
or `Conflict` — the last meaning a different **Source** already owns that name
(`SkillInstallStatus`). It is computed from the filesystem at read time, never stored.
_Avoid_: Enabled, active, linked (that is **Link status**, a different axis).

**Blocklist**:
The `.agm_uninstalled` file kept beside the **Central store**'s `skills/` directory, listing
names the user explicitly uninstalled so a later `agm source update` does not re-install them.
_Avoid_: Denylist, ignore file, exclusions.

**Preload chars**:
The character cost an **Item** imposes on a **Tool**'s context before use: for a **Skill**, the
`name` + `description` values in its `SKILL.md` YAML frontmatter; for an **Agent** or
**Command**, the whole file's character count. Shown in the **Source Manager**.
_Avoid_: Token count, size, weight.

**Shell**:
The unified TUI process (`src/tui/shell.rs`) opened by bare `agm`, `agm tool`, or `agm source`.
Owns the terminal lifecycle and hosts the **Tool Manager** and **Source Manager** as two
screens switched with `Tab` / `Shift+Tab`.
_Avoid_: App, main loop, TUI (ambiguous between the shell and a screen).

**Tool Manager**:
The **Shell** screen focused by `agm tool` (and the default when opening bare `agm`) — one row
group per **Tool**, showing **Link status** per **Feature** and allowing edit and toggle. Lives
in `src/tui/tool.rs`.
_Avoid_: Tool TUI, tool pane.

**Source Manager**:
The **Shell** screen focused by `agm source` — a tree of **Source**s and their **Item**s, with
search, multi-select, and install/uninstall. Lives in `src/tui/source.rs`.
_Avoid_: Source TUI, skills TUI.

**Migration**:
The one-way move performed when AGM adopts a **Tool** that already had real content where a
**Link** belongs: the content moves into `source/agm_tools/<tool key>/`, becoming a **Source**
of kind `Migrated`, and the original path is replaced by a **Link**.
_Avoid_: Import, adoption, takeover.
