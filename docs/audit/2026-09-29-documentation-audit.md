# Documentation audit — 2026-09-29
Status: Resolved (2026-09-29)

Two-pass sweep (wens-dev-principles docs 19) of every living document against the tree at
`602695e` (v0.16.0), folding in the prompt audit of the agent instruction files
(`docs/tmp/claude-scratch/prompt-audit-2026-09-29.md`, to be landed beside this record as
[2026-09-29-prompt-audit.md](2026-09-29-prompt-audit.md)). Code is cited by file + symbol.

Dispositions: **doc** = corrected in this audit's commit; **backlog** = code defect, filed in
[`../plan/BACKLOG.md`](../plan/BACKLOG.md), not fixed here (docs describe current behavior,
including the defect); **decision** = needs the owner's choice.

## Pass 1 — structure

| ID | Finding | Evidence | Disposition |
|---|---|---|---|
| S1 | No living backlog; `CONTEXT.md` has no backlog row or reading-order entry; audit-folder description omits assessment/verification runs | `docs/plan/` listing; `CONTEXT.md` folder table | doc |
| S2 | Dead `docs/superpowers/…` / `docs/specs/…` paths written as inline code in six frozen records, missed by the 2026-09-02 restructure | `spec/2026-04-01-tui-redesign.md` (mockups line), `plan/2026-03-20-windows-platform-support.md`, `plan/2026-03-20-prompt-blocked-and-display-fixes.md`, `plan/2026-03-21-skills-refactor.md`, `plan/2026-04-01-tui-redesign.md`, `plan/2026-05-20-cli-refactor-and-source-improvements.md` (`**Spec:**` lines) | doc (mechanical path repair, docs 7) |
| S3 | `CHANGELOG.md` heading spelled `## Unreleased`; `[v0.5.0]`/`[v0.4.0]` sit below `[v0.3.1]`/`[v0.3.0]`; `[0.1.1]`/`[0.1.0]` lack the `v` every tag uses | `CHANGELOG.md`; `git tag` | doc |
| S4 | `CONTEXT.md` describes `docs/tmp/` as plain scratch; `.gitignore` actually ignores all of it except `archive/` | `.gitignore` | doc |
| S5 | `file:line` citations in reference (docs 18) | `cli.md` §`agm tool link` (`link_all`, `yes = true`), §Feature skip (`Config::resolved_link_path`) | doc |

## Pass 2 — accuracy

### Reference vs code

| ID | Document § | Claim → actual | Disposition |
|---|---|---|---|
| A1 | `cli.md` §`agm source list` | Says commands and **Preload chars** are printed → `source_list` prints skills and agents only, no preload chars | doc + backlog B3 |
| A2 | `cli.md` §`agm source add` | Omits that a URL source also installs every discovered **Agent** without prompting (`source_add` → `scan_agents`/`install_agent`) | doc |
| A3 | `config.md` §Pre-registered tools | Root `editor` key described inside the tools section, absent from the schema tables | doc |
| A4 | `config.md` §`[agm]` | Serde-default note omits `disabled` (`AgmConfig.disabled` is `#[serde(default)]`) | doc |
| A5 | `config.md` §Path semantics, `glossary.md` **Config dir** | Documents `ToolConfig::resolve_path` (`$VAR` expansion, `/`-means-absolute) — dead code, test-only. Runtime resolver is `ToolState::get_group_files`: absolute or `~`-prefixed → `expand_tilde`, else `config_dir.join`; no `$VAR`. Absolute entries work for `settings`/`mcp` too, not only `auth` | doc + backlog B7 |
| A6 | `sources.md` §Update | "skipped for pull but still re-synced" → `update_all_with_progress` skips `local`/`agm_tools` for both | doc |
| A7 | `sources.md` §Migration | Says `agm tool link` migrates agents and commands → only skills migrate; CLI deletes a real `agents/` dir and never links `commands`; `migrate_agents_dir_quiet`/`migrate_commands_dir_quiet` are reached only from the Tool Manager (`handle_blocked_link`). `cli.md` already states this correctly — the two reference files contradict each other | doc + backlog B1, B2 |
| A8 | `sources.md` §Add a source, §Machine-checked | `clone_or_pull_routes_errors_through_callback_not_stdout` presented as run by `cargo test` → it is `#[ignore]` | doc + backlog B10 |
| A9 | `architecture.md` §Layering | Diagram puts `skills.rs` above `linker.rs` → `skills.rs` calls `platform` directly; `linker` is used by `main.rs`, `status.rs`, `tui/` | doc |
| A10 | `architecture.md` §Data flow — `agm tool link` | "per Feature" and "prune broken links" → prunes skills/agents only; links skills, agents, prompt only | doc |
| A11 | `architecture.md` §Build and test | `tests/source_ops.rs` said to use `assert_cmd` → it calls `agm::skills` directly | doc |
| A12 | `linking.md` §Link status | CLI rendering column omits `→ <path>` on `Linked`/`Blocked`/`Missing`; contradicts `cli.md` and `status.rs` | doc |
| A13 | `linking.md` §Machine-checked | "every `LinkStatus` branch and the decision table" → `Broken`, create-refusal, and remove-refusal branches untested in `linker::tests` | doc + backlog B8 |
| A14 | `KEYMAP.md` §Source Manager | `e` "opens the source directory, skill directory…" → source rows do nothing; skill rows open `SKILL.md` (`App::open_editor`) | doc |
| A15 | `KEYMAP.md` §Popups | `⏎`/`␣` close every popup → Log popup closes on `o`/`Esc` only (callers drop `PopupAction::Close`) | doc + backlog B5 |
| A16 | `KEYMAP.md` §Tool Manager | `e` on any central row opens the inline path editor → the prompt row opens the file in the editor (`ToolApp::handle_edit`) | doc |
| A17 | `KEYMAP.md` §The Esc contract | Steps 5 and 6 are one press: clearing the search also clears the status (`App::handle_key`, `Esc`) | doc |
| A18 | `KEYMAP.md` §Machine-checked | Help panel called authoritative, but `build_help_lines` lists `Ctrl+C` for the Source screen only while `shell::run` handles it on both | doc + backlog B6 |
| A19 | `tui.md` §Common shape | Title `agm — <chip>` and a two-line footer → title is `agm — [Tool] · Source` / `Tool · [Source]`; footer has one content line that status/progress replaces; progress and selection count are Source-screen only | doc |
| A20 | `tui.md` §Log | One log implied → each screen owns its own 500-entry `LogBuffer` | doc |
| A21 | `tui.md` §Machine-checked | "None" → `shell.rs` is unharnessed, but `popup`, `help`, `style`, `text_input`, `background`, `tool`, `source` have unit tests | doc |
| A22 | `glossary.md` **Feature** | Definition says "categories" while `_Avoid_` bans "Category"; the `Category` enum (Source Manager's Skills/Agents/Commands headers) has no entry | doc |
| A23 | `releasing.md` | Supported platforms and artifact names omit the two Windows MSVC targets and `.zip` artifacts built by `release.yml` | doc |
| A24 | `reference/README.md` | Claims `architecture.md` is machine-checked by "the corresponding module"; no such module | doc |
| A25 | `reference/README.md`, `cli.md` §`NO_COLOR`, `architecture.md` | Pre-v0.16.0 "both TUIs" / "the two managers" wording | doc |
| A26 | `README.md` | Omits the **Command** feature throughout (features, TUI capabilities, default dirs); says `agm config` uses `$EDITOR` first (`get_editor` checks `config.editor` first); `source add <url>` where the argument also takes `user/repo` or a local path | doc |

### Shipped but undocumented

| ID | Feature (changelog) | Disposition |
|---|---|---|
| U1 | Shallow clone `--depth 1` (v0.12.0) — `clone_or_pull` | doc (`sources.md`) |
| U2 | Duplicate-name reporting in bulk install and info popups (v0.12.0) — `duplicate_name_count` | doc (`tui.md`) |
| U3 | Globally disabled **Feature**s greyed in the Source Manager, actions guarded (v0.7.2) | doc (`tui.md`) |
| U4 | Bulk direction rule: install-all if any selected item is not installed, else uninstall-all (v0.13.0) — `start_bulk_selection` | doc (`tui.md`) |

### Agent instruction files (prompt audit)

| ID | Finding | Disposition |
|---|---|---|
| P1 | "No `#[cfg]` outside `platform.rs`" contradicted by `linker::remove_link`, `linker::remove_link_quiet`, `paths::contract_tilde` (production) and two `#[cfg(unix)]` tests in `skills.rs`; same claim in ADR 0003 and `architecture.md` | decision → keep the rule; backlog B11 |
| P2 | "Nothing reachable from `src/tui/` prints" contradicted by the `eprintln!` in `Config::resolved_link_path`, called from `tui/tool.rs` | decision → keep the rule; backlog B11 |
| P3 | `GEMINI.md` requires ADRs for deviations from `wens-dev-principles` without saying where it lives (`github.com/superyngo/wensdev`, `skills/wens-dev-principles/`) | doc |
| P4 | No `CLAUDE.md` / `AGENTS.md`; Claude Code and Codex sessions get no repo conduct | decision → conduct moved to `AGENTS.md`; `CLAUDE.md` and `GEMINI.md` import it |
| P5 | `.github/copilot-instructions.md` "Where things are" omits `releasing.md` | doc |

## Resolution

Every **doc** item was corrected in the commit that landed this record. Code defects are backlog
rows B1–B8, B10–B12 (B9 unused). B12 (footer never draws progress or selection count) surfaced
while correcting A19. `docs/tmp/` stays gitignored except `archive/`
(wens-dev-principles docs 12 is `CONSIDER`); `CONTEXT.md` now says so.

## Checked and clean

- Every working record has a valid `Status:` on line 2; every folder index row resolves; every
  `.md` is indexed; `adr/README.md` matches the six ADR files.
- `architecture.md` module map lists exactly the modules in `src/` and `src/tui/`.
- `linking.md` platform link kinds; `sources.md` discovery, install/uninstall blocklist, preload
  counting, delete and rename behavior.
- Neither agent instruction file restates reference; they never disagree with each other.
