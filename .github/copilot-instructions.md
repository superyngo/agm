# AGM — Copilot Instructions

Read [`../CONTEXT.md`](../CONTEXT.md) first: it indexes every document in this repo. Start with
[`../docs/reference/glossary.md`](../docs/reference/glossary.md) for the vocabulary, then
[`../docs/reference/architecture.md`](../docs/reference/architecture.md) for the module map and
data flow.

This file holds repo conduct only and deliberately does not restate reference material.

## Build, test, run

```sh
cargo build                 # debug
cargo build --release       # → target/release/agm
cargo test                  # unit + integration
cargo test <name>           # single test, e.g. cargo test test_scan_skills_single
cargo test -- --nocapture   # show println! output
```

## Where things are

| Looking for | Read |
|---|---|
| Vocabulary | [`../docs/reference/glossary.md`](../docs/reference/glossary.md) |
| Module map, layering, invariants | [`../docs/reference/architecture.md`](../docs/reference/architecture.md) |
| Commands, flags, env vars | [`../docs/reference/cli.md`](../docs/reference/cli.md) |
| `config.toml` schema and path rules | [`../docs/reference/config.md`](../docs/reference/config.md) |
| Link statuses and decision tables | [`../docs/reference/linking.md`](../docs/reference/linking.md) |
| Source/skill/agent/command behavior | [`../docs/reference/sources.md`](../docs/reference/sources.md) |
| TUI structure and keys | [`../docs/reference/tui.md`](../docs/reference/tui.md), [`../docs/reference/KEYMAP.md`](../docs/reference/KEYMAP.md) |
| Why the design is what it is | [`../docs/adr/README.md`](../docs/adr/README.md) |

## Conduct

- Behavior change → update the matching `docs/reference/` file in the same commit.
- New term → add its glossary entry in the same commit.
- Landed documents in `docs/spec|plan|debug|audit/` are frozen; only their `Status:` line may
  change.
- New document → add its row to the folder's `README.md` in the same commit.
- Keep platform `#[cfg]` inside `src/platform.rs`; keep printing out of anything reachable from
  `src/tui/`.
- Prefer the smallest change that solves the problem, and match the surrounding style.
