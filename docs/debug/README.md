# Debug notes

Handoff notes from investigations, with repro material. Every file here is a historical record:
frozen once resolved, dated by when it was written, never rewritten. Current behavior lives in
[`../reference/`](../reference/README.md).

A note may own a sibling directory with the same basename
(`2026-09-02-foo.md` + `2026-09-02-foo/`) holding repro scripts, fixtures, and captured output.
The directory is indexed from inside the note, never from this file. Scripts there must run
standalone — standard library only, no `cargo build`, no import from `src/` — so they still work
after the code moves on.

## In progress

- [`2026-11-17-windows-nested-feature-link.md`](2026-11-17-windows-nested-feature-link.md) —
  Windows release CI fails `link_creates_missing_parent_of_nested_feature_dir`; root cause not
  yet confirmed, repro steps handed off for a real Windows run.

## Landed

_None yet._
