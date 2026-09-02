# CONTEXT

Entry point for all documentation. Root-level files (`README.md`, `CHANGELOG.md`, `LICENSE`)
stay here; everything else lives under `docs/`. The agent instruction files (`GEMINI.md`,
`.github/copilot-instructions.md`) carry repo conduct only and point back here.

| Folder | Holds | Canonical? | Lifecycle |
|---|---|---|---|
| [`docs/reference/`](docs/reference/README.md) | Current behavior: glossary, CLI surface, config schema, linking, sources, TUI, keymap, release process | Yes — the only source of truth | Kept in sync with the code |
| [`docs/adr/`](docs/adr/README.md) | Decisions that were expensive to reach and would be expensive to reverse | No — historical | Never edited; superseded by a new ADR |
| [`docs/spec/`](docs/spec/README.md) | Design records written before implementation | No — historical | Frozen once approved; only `Status:` changes |
| [`docs/plan/`](docs/plan/README.md) | Task-by-task implementation plans derived from a spec | No — historical | Frozen once shipped; only `Status:` changes |
| [`docs/debug/`](docs/debug/README.md) | Handoff notes from investigations, with repro scripts | No — historical | Frozen once resolved; only `Status:` changes |
| [`docs/audit/`](docs/audit/README.md) | Point-in-time sweeps for bugs, dead code, inconsistency | No — historical | Frozen once findings are addressed; only `Status:` changes |
| `docs/tmp/` | Scratch | No | Archived to `tmp/archive/YYYY-MM.tar.gz` when stale |

## Reading order

1. [`docs/reference/glossary.md`](docs/reference/glossary.md) — the vocabulary every other file uses.
2. [`docs/reference/README.md`](docs/reference/README.md) — the subsystem map.
3. [`docs/adr/README.md`](docs/adr/README.md) — why the shape is what it is.
4. `CHANGELOG.md` — what changed recently.

## Conventions

This layout follows the `wens-dev-principles docs` domain. Two consequences worth stating up
front:

- A document in `spec/`, `plan/`, `debug/`, or `audit/` is **frozen**. Do not edit it to match
  new behavior — write a new document and point the old one's `Status:` line at it.
- A new term goes into [`docs/reference/glossary.md`](docs/reference/glossary.md) in the same
  commit that introduces it, and code identifiers use the glossary spelling.
