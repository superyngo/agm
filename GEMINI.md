# AGM — Agent Instructions

Read [`CONTEXT.md`](CONTEXT.md) first. It indexes every document in this repo. Start with
[`docs/reference/glossary.md`](docs/reference/glossary.md) — use its terms in code, output, and
commit messages — then [`docs/reference/architecture.md`](docs/reference/architecture.md).

This file holds repo conduct only. It deliberately does not restate reference material; if a
fact about how AGM behaves is missing here, it belongs in `docs/reference/` and is linked from
that folder's `README.md`.

## Commands

```sh
cargo build                 # debug
cargo build --release       # → target/release/agm
cargo test                  # unit + integration
cargo test <name>           # single test
cargo test -- --nocapture   # show println! output
```

## Conduct

- **Reference is the source of truth.** Changing behavior means updating the matching file in
  `docs/reference/` in the same commit. Never document behavior in this file.
- **Records freeze.** Files in `docs/spec/`, `docs/plan/`, `docs/debug/`, `docs/audit/` are
  historical. Do not edit a landed document except its `Status:` line — write a new one and
  point the old `Status:` at it.
- **New term → glossary.** Add the entry in the same commit that introduces the term.
- **Expensive decision → ADR.** One file in `docs/adr/`, never edited afterward. Deviating from
  a MUST principle of `wens-dev-principles` also requires an ADR citing it by domain and number.
- **New document → its folder `README.md` in the same commit.** An index that omits a file is a
  bug.
- **Errors:** `anyhow::Result`, with context that names the path or tool involved.
- **No printing below the interface layer.** Anything reachable from `src/tui/` must return
  messages instead of writing to stdout — see
  [`docs/adr/0005-ratatui-tui-as-primary-interface.md`](docs/adr/0005-ratatui-tui-as-primary-interface.md).
- **No `#[cfg(unix)]` / `#[cfg(windows)]` outside `src/platform.rs`** — see
  [`docs/adr/0003-platform-abstraction-for-windows-links.md`](docs/adr/0003-platform-abstraction-for-windows-links.md).
- **Never widen destruction.** `linker` refuses to overwrite real files and wrongly-targeted
  links; keep destructive handling in the caller, and keep it announced.
- **Tests:** unit tests beside the code with `tempfile`; CLI behavior in `tests/` with
  `assert_cmd`. Verify link changes on the real binary, not only in unit tests.
- **Commit** code, `CHANGELOG.md`, and doc updates together, one self-contained commit per task.
