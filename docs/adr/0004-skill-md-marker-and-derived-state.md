# 0004 — `SKILL.md` is the only marker, and install state is derived from the filesystem

Date: 2026-03-21

## Context

AGM needs to know what is installable inside an arbitrary git repository, and which of those
things the user currently has installed. Nothing in the ecosystem provides a manifest: a
"skills repo" is a directory of directories, laid out however its author felt.

## Decision

Two rules, both filesystem-only:

1. **Discovery by marker.** A directory is a **Skill** if it contains `SKILL.md`; discovery
   recurses to depth 3 and stops descending as soon as it finds one. An **Agent** is any `.md`
   file directly inside a source's `agents/`; a **Command** likewise inside `commands/`.
2. **State is derived, never stored.** **Install status** is computed on every scan by reading
   the **Central store** and comparing link targets against the **Source**'s path. There is no
   index, database, or manifest of installed items. The name is the identity, so two sources
   cannot both install the same name — the loser reads as `Conflict`.

One deliberate exception to rule 2: uninstalling a **Skill** writes its name to the
**Blocklist** (`.agm_uninstalled`).

## Alternatives rejected

- **A manifest inside each source.** Would need every upstream author's cooperation. Sources
  are third-party repos AGM does not control.
- **An AGM-side index of installed items.** Two sources of truth. The user moves, renames, or
  deletes things behind AGM's back; an index goes stale silently, while a re-scan cannot.
- **Content sniffing / heuristics** (frontmatter shape, directory naming). Unbounded guessing
  with no rule the user can predict.
- **No blocklist — just re-derive everything.** Rejected because `agm source update` installs
  newly appeared skills. Without a record of intent, every update resurrects exactly the skills
  the user removed.

## Consequences

- Nested skills are invisible: once a directory has `SKILL.md`, its subdirectories are not
  searched.
- Naming collisions across sources are structural, not incidental. `Conflict` is a first-class
  **Install status** and the user resolves it by uninstalling the other one.
- Every list operation costs a filesystem walk. Acceptable at this scale, and it means the
  display can never disagree with the disk.
- The **Blocklist** is asymmetric: only skills are blocklisted, because only skills are
  auto-installed by update. It lives beside `skills/`, not inside it, so it is never mistaken
  for an installed skill.
