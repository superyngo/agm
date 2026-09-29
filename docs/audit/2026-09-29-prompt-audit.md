# Prompt audit — 2026-09-29
Status: Resolved (2026-09-29)

Run via `/claude-api prompt-audit`, at `602695e` (v0.16.0).

## Assumptions

- **Scope:** the whole working directory's prompt surface. Skipped: `~/.claude/CLAUDE.md` (user-level, not named in the request), `.claude/settings.local.json` (settings file; not read by design).
- **Target model:** no model named in the request, and the repository has no Claude API code → Claude Opus 5.5 (the model running this audit).
- **Non-Anthropic readers:** `GEMINI.md` is read by the Gemini CLI / Antigravity agent, and `.github/copilot-instructions.md` by GitHub Copilot. Both are audited as generic agent instruction files. Nothing here proposes switching either to Anthropic tooling.

## Inventory

| File | Kind |
|---|---|
| `GEMINI.md` (43 lines) | Agent instruction file |
| `.github/copilot-instructions.md` (42 lines) | Agent instruction file |
| `CONTEXT.md` | Doc index both files point at first (read only for cross-checks) |

No `CLAUDE.md`, `AGENTS.md`, skills, commands, subagents, system prompts, tool definitions, or request-building code in the repo (`src/` is a Rust CLI/TUI with no LLM calls). `.crush/` holds only a generated `.gitignore`.

## Summary

Both files are short, carry their reasons, and point to `docs/reference/` rather than restating it. The real findings are two rules that the shipping code contradicts. First, both files (and ADR 0003 and `architecture.md`) say `#[cfg(unix)]`/`#[cfg(windows)]` appears only in `src/platform.rs`, but three production `#[cfg(windows)]` sites sit outside it. Second, both files say nothing reachable from the TUI prints, but `Config::resolved_link_path` calls `eprintln!` and the TUI calls it. Both are flagged, not diffed. Each fix means either changing code or loosening a stated rule through a new ADR, and that decision is yours.

Counts: Group 1 (dated prompt text): 0. Group 2 (brittle config files): 2 high-confidence flags and 1 low-confidence flag. Group 3 (tool descriptions): not applicable. Group 4 (request config and architecture): not applicable.

## Findings

### 1. `#[cfg]` boundary rule contradicted by the code. High confidence, action: flag

- **Location:** `GEMINI.md:37-38`, `.github/copilot-instructions.md:40`. The same claim appears in `docs/reference/architecture.md:44` and `docs/adr/0003-platform-abstraction-for-windows-links.md:13,43`.
- **Evidence:** "No `#[cfg(unix)]` / `#[cfg(windows)]` outside `src/platform.rs`" / "Keep platform `#[cfg]` inside `src/platform.rs`"
- **Contradicting code:** `src/linker.rs:179` and `:257` (`#[cfg(windows)]` / `#[cfg(not(windows))]` branches in `remove_link` / `remove_link_quiet`), and `src/paths.rs:89` (`contract_tilde` separator swap). Test-only gating: `src/skills.rs:1906`, `:2390` (`#[cfg(unix)]` on tests).
- **Pattern:** Group 2, volatile specifics (a claim the repository contradicts). Also 1d, unenforced instruction: no check enforces the rule, and the violations predate the rule text (added in 229148f; the `cfg` sites date from 4d6271b and 7eaceb1).
- **Why it matters:** An agent that reads the rule will either refuse to touch these functions' platform branches or "fix" them without being asked. An agent that reads the code will conclude the rule is not real.
- **Sweep:** `rg 'cfg!\(|#\[cfg\((not|any|all|target_|unix|windows)' src tests` (excluding `platform.rs`) found only the sites above. There are no `cfg!()` macro uses.
- **Your decision:** (a) move the three production branches behind `platform::` helpers so the rule becomes true. ADR 0003:43 already calls a `#[cfg]` elsewhere a defect, so (a) is what the ADR requires. Or (b) allow test gating and name the exceptions. ADRs are never edited (`docs/adr/README.md:4`, `CONTEXT.md:10`), and `GEMINI.md:29-30` requires a new ADR for any deviation, so (b) means writing a new ADR that supersedes 0003's boundary claim, then updating `GEMINI.md`, the Copilot file, and `architecture.md:44`. Option (b) loosens a stated rule, so this audit does not propose it as a diff.

### 2. "No printing below the interface layer" contradicted by the code. High confidence, action: flag

- **Location:** `GEMINI.md:34-36`, `.github/copilot-instructions.md:40-41`.
- **Evidence:** "Anything reachable from `src/tui/` must return messages instead of writing to stdout" / "keep printing out of anything reachable from `src/tui/`"
- **Contradicting code:** `Config::resolved_link_path` has an `eprintln!` warning at `src/config.rs:294` (dated 72b66d2, 2026-04-04). The TUI calls it at `src/tui/tool.rs:748`, `:2085`, and `:2462`. It writes to stderr, not stdout, but it is still printing while ratatui owns the terminal. It fires when a tool's link path resolves to its own `config_dir`.
- **Pattern:** Group 2, volatile specifics (a claim the repository contradicts). Also 1d, unenforced instruction.
- **Your decision:** make `resolved_link_path` return the warning to its caller (a code fix, out of this audit's scope). Or accept stderr output as an exception, which loosens the rule and needs the same ADR route as Finding 1 (ADR 0005's "nothing prints").

### 3. Dependency on an out-of-repo skill. Low confidence, action: flag

- **Location:** `GEMINI.md:29-30` (and `CONTEXT.md:26`).
- **Evidence:** "Deviating from a MUST principle of `wens-dev-principles` also requires an ADR citing it by domain and number."
- **Pattern:** Group 2, volatile specifics (a reference outside the repository, so not contradicted by its absence).
- **Note:** Gemini/agy and Copilot may not have that user-level skill, so the rule can't be applied by the agents that read this file. A one-line pointer to where the principles live would make the rule usable. Leave it as-is if the file is only ever used on your own machine.

## Checked and clean

- Every linked path in both files exists. `cargo test test_scan_skills_single` resolves (`src/skills.rs:1463`). `anyhow`, `tempfile`, and `assert_cmd` are in `Cargo.toml`.
- Apart from Finding 2, the printing rule holds. `src/tui/` reaches only `linker::*_quiet` and `check_link`, and `skills.rs`, `paths.rs`, `platform.rs`, and `editor.rs` don't print. `status.rs` and `init.rs` print, but the TUI doesn't call them.
- The two files differ in coverage (Copilot's omits the errors, destruction, tests, commit, and ADR rules) but never disagree. Keep-list item 8 applies, so this is not a finding.
- No pressure language, thinking scaffolds, choreography, or fossils. Each emphatic rule carries its reason.

## Proposed diff

Empty. All three findings are `flag` items that need a decision from you (see Step 6 of the audit guide: flag/low-confidence items stay out of the diff).

## Out of scope (observation only)

The repo has no `CLAUDE.md` / `AGENTS.md`, so Claude Code and Codex sessions here don't receive the conduct rules in `GEMINI.md`.
