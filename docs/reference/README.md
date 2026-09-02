# Reference

Current behavior only. Anything historical — a superseded design, a shipped plan, a resolved
investigation — lives in [`../spec/`](../spec/README.md), [`../plan/`](../plan/README.md),
[`../debug/`](../debug/README.md), or [`../audit/`](../audit/README.md), not here.

- **[glossary.md](glossary.md)** — canonical vocabulary; read first.
- **[architecture.md](architecture.md)** — module map, layering, data flow, build and test commands.
- **[cli.md](cli.md)** — command surface, flags, per-command behavior, environment variables.
- **[config.md](config.md)** — `config.toml` schema, path semantics, pre-registered tools, `agm init`.
- **[linking.md](linking.md)** — link kinds per platform, `LinkStatus`, create/remove decision tables.
- **[sources.md](sources.md)** — source/skill/agent/command discovery, install, update, rename, migration.
- **[tui.md](tui.md)** — the two managers, popups, selection model, background work, `NO_COLOR`.
- **[KEYMAP.md](KEYMAP.md)** — every key binding across both TUIs and all popups.
- **[releasing.md](releasing.md)** — tagging and the release workflow.

Machine-checked: the CLI surface claims in `cli.md` by `tests/cli.rs`; the source operations in
`sources.md` by `tests/source_ops.rs`; the schema, path, and link claims in `config.md`,
`linking.md`, and `architecture.md` by the `#[cfg(test)]` unit tests in the corresponding module.
`tui.md` and `KEYMAP.md` are **not** machine-checked — the in-app Help panel (`?`) is generated
from the code and wins any disagreement.

See [`../adr/`](../adr/README.md) for the decisions behind this shape.
