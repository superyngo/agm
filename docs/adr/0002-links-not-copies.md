# 0002 — One central store, reached by links rather than copies

Date: 2026-02-14

## Context

The same prompt, skills, agents, and commands need to be visible to every installed AI coding
CLI, each of which insists on its own directory layout and its own instruction filename
(`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`). The user edits this material constantly.

## Decision

Keep exactly one real copy in a **Central store** (`~/.local/share/agm/` by default) and give
each **Tool** a **Link** to it from inside its own **Config dir**. AGM's job is to create,
verify, and remove those links; it never copies content into a tool.

The single exception is `agm tool unlink`: after removing a link it copies the central content
back into the tool's path, so a tool keeps working after AGM steps out.

## Alternatives rejected

- **Copy on write, sync on command.** Requires a sync direction, conflict resolution, and a
  daemon or a habit of running `agm sync`. Editing a skill in one tool and forgetting to sync
  silently diverges the copies — the exact failure the tool exists to prevent.
- **A daemon watching the filesystem.** Far more machinery, a background process to install and
  debug, and still ambiguous about which copy wins.
- **Per-tool configuration pointing at a shared path.** Only works for the minority of tools
  that accept a configurable path; the rest hardcode their layout.

## Consequences

- An edit is instantly visible to every tool, because there is only ever one file.
- AGM must be conservative about what it overwrites. A real file or a wrongly-targeted link is
  never silently replaced: `linker::create_link` refuses `Blocked` and `Wrong` outright, and
  destructive handling — migrating a populated skills directory, backing up a non-empty prompt
  to `<name>.<timestamp>.bak` — is the caller's explicit, announced decision.
- Pre-existing content in a tool has to go somewhere, which is why **Migration** into
  `source/agm_tools/<tool key>/` exists.
- The tool inherits filesystem link semantics, including their platform differences — see
  [0003](0003-platform-abstraction-for-windows-links.md).
- Removing a link is not a true undo: `unlink` leaves copies, not the tool's original content.
