# 0001 — Tools are registered in config, not in code

Date: 2026-02-14

## Context

AGM manages an open-ended and fast-moving set of AI coding CLIs. Each one differs only in
data: where its config directory is, what its instruction file is called, which files hold
settings, credentials, and MCP configuration. New tools appear faster than releases ship, and
users run private or pre-release tools AGM has never heard of.

## Decision

A **Tool** is defined entirely by a `[tools.<key>]` table in `config.toml`. `ToolConfig` has no
per-tool behavior, no trait implementations, and no match arms on **Tool key**. A tool is
considered *installed* if and only if its `config_dir` exists on disk
(`ToolConfig::is_installed()`). `Config::default_config` seeds seven well-known tools purely as
convenience data.

Adding a tool therefore requires no code change and no new binary.

## Alternatives rejected

- **A `Tool` trait with one implementation per tool.** Correct if tools differed
  *behaviorally*. They do not — they differ in paths and filenames. The trait would have been
  seven identical implementations parameterized by strings, plus a release cycle standing
  between the user and a new tool.
- **A bundled registry file updated per release.** Same release-cycle problem, and it makes the
  user's own tools second-class.
- **Auto-detection by scanning the home directory.** Guessing which directory belongs to which
  agent is unbounded and silently wrong; `config_dir` existence is a rule the user can read.

## Consequences

- New tool support is a documentation and config concern. `docs/reference/config.md` lists the
  pre-registered set; nothing in `src/` enumerates tools.
- Tools iterate in alphabetical **Tool key** order everywhere, because `Config.tools` is a
  `BTreeMap`. Output ordering is stable for free.
- A mistyped path is a user-visible mistake rather than a compile error, so path resolution has
  to be defensive: a **Feature** whose resolved link path collides with the **Config dir** is
  refused with a warning rather than acted upon.
- Because the registry is data, a config written by an older AGM must keep loading. New
  `[agm]` and `[tools.*]` fields carry serde defaults.
