# AGM Major Refactor — Design Specification

**Date:** 2026-04-01
**Version:** 0.6.0 (proposed)
**Scope:** Config schema restructuring, files feature removal, agents feature addition, command renaming, TUI overhaul, 3 new default tools

## 1. Problem Statement

AGM v0.5.0 manages shared prompts, skills, and config files across AI coding tools. This refactor addresses:

1. **Unnecessary complexity** — the `files` centralization feature adds cognitive overhead with little benefit. Remove it.
2. **Agents support** — AI tools now support subagent dispatch via `.md` files in `agents/` directories. AGM needs to discover and manage these alongside skills.
3. **Naming clarity** — `skill_repos` is misleading since repos contain both skills and agents. Rename to `source_repos`. The `agm skills` command manages sources, not just skills—rename to `agm source`.
4. **Tool coverage** — Codex, Pi, and Crush are now mainstream AI coding CLIs that should be included in defaults.
5. **TUI usability** — The interactive manager needs collapsible groups, agents management, fuzzy search, and keyboard shortcuts for efficient operation.

## 2. Config Schema Changes

### 2.1 CentralConfig

```rust
pub struct CentralConfig {
    pub prompt_source: String,        // unchanged: ~/.local/share/agm/prompts/MASTER.md
    pub skills_source: String,        // unchanged: ~/.local/share/agm/skills
    pub agents_source: String,        // NEW: ~/.local/share/agm/agents
    pub source_dir: String,           // unchanged: ~/.local/share/agm/source
    pub source_repos: Vec<String>,    // RENAMED from skill_repos
}
```

**Removed fields:** `files_base`, `files`

### 2.2 ToolConfig

```rust
pub struct ToolConfig {
    pub name: String,
    pub config_dir: String,
    pub settings: Vec<String>,
    pub auth: Vec<String>,
    pub prompt_filename: String,
    pub skills_dir: String,
    pub agents_dir: String,           // NEW
    pub mcp: Vec<String>,
}
```

**Removed fields:** `files`

### 2.3 TOML Representation

```toml
[central]
prompt_source = "~/.local/share/agm/prompts/MASTER.md"
skills_source = "~/.local/share/agm/skills"
agents_source = "~/.local/share/agm/agents"
source_dir = "~/.local/share/agm/source"
source_repos = []
```

### 2.4 Default agents_dir

All tools default to `agents_dir = "agents"`. If a tool does not support agents, the user can set it to `""` and the link step will skip it.

## 3. Default Tools (7 total)

### 3.1 Existing Tools (updated with agents_dir)

```toml
[tools.claude]
name = "Claude Code"
config_dir = "~/.claude"
settings = ["settings.json"]
auth = []
prompt_filename = "CLAUDE.md"
skills_dir = "skills"
agents_dir = "agents"
mcp = ["mcp.json"]

[tools.gemini]
name = "Gemini CLI"
config_dir = "~/.gemini"
settings = ["settings.json"]
auth = ["oauth_creds.json", "google_accounts.json"]
prompt_filename = "GEMINI.md"
skills_dir = "skills"
agents_dir = "agents"
mcp = []

[tools.copilot]
name = "Copilot CLI"
config_dir = "~/.copilot"
settings = ["config.json"]
auth = []
prompt_filename = "AGENTS.md"
skills_dir = "skills"
agents_dir = "agents"
mcp = ["config.json"]

[tools.opencode]
name = "OpenCode"
config_dir = "~/.config/opencode"
settings = ["opencode.json"]
auth = []
prompt_filename = "AGENTS.md"
skills_dir = "skills"
agents_dir = "agents"
mcp = ["opencode.json"]
```

### 3.2 New Tools

```toml
[tools.codex]
name = "Codex"
config_dir = "~/.codex"
settings = ["config.toml"]
auth = ["auth.json"]
prompt_filename = "AGENTS.md"
skills_dir = "skills"
agents_dir = "agents"
mcp = ["config.toml"]

[tools.pi]
name = "Pi"
config_dir = "~/.pi/agent"
settings = ["settings.json"]
auth = ["auth.json"]
prompt_filename = "AGENTS.md"
skills_dir = "skills"
agents_dir = "agents"
mcp = []

[tools.crush]
name = "Crush"
config_dir = "~/.config/crush"
settings = ["crush.json"]
auth = ["crush.json"]
prompt_filename = "AGENTS.md"
skills_dir = "skills"
agents_dir = "agents"
mcp = ["crush.json"]
```

### 3.3 Tool Path Validation Results

| Tool | Config Dir | Verified On-Disk | Settings File | Auth File | MCP File | Notes |
|------|-----------|-----------------|---------------|-----------|----------|-------|
| Codex | `~/.codex` | ✅ Exists | `config.toml` ✅ | `auth.json` ✅ | `config.toml` (same as settings) | MCP in `[mcp_servers.*]` section |
| Pi | `~/.pi/agent` | ✅ Exists | `settings.json` ✅ | `auth.json` ✅ | None | No MCP config mechanism found |
| Crush | `~/.config/crush` | ✅ Exists | `crush.json` ✅ | `crush.json` (same) | `crush.json` (same) | Single file: providers + API keys + MCP |

## 4. Files Feature Removal

### 4.1 What Gets Removed

- **`src/files.rs`** — entire module deleted
- **`FileStatus` enum** and all file link/check/centralize logic
- **`files_base`** field from `CentralConfig`
- **`files`** field from both `CentralConfig` and `ToolConfig`
- **`init.rs`** — remove `~/.local/share/agm/files` directory creation
- **`main.rs`** — remove file processing from `link`, `unlink` commands; remove `central` files handling
- **`status.rs`** — remove file status display

### 4.2 No Migration

Breaking change. Users with centralized files keep their `~/.local/share/agm/files/` directory intact but AGM stops managing it. They should manually restore symlinked files if needed.

## 5. Agents Feature

### 5.1 Central Agents Directory

```
~/.local/share/agm/agents/
  code-reviewer.md → ../../source/superpowers/agents/code-reviewer.md
  design-critic.md → ../../source/myskills/agents/design-critic.md
```

Agents are single `.md` files (not directories). Each symlink in the central agents dir points to the source file.

### 5.2 Discovery

During `scan_all_sources()`, for each source directory:
1. Check for `agents/` as a direct child directory (not recursive — unlike skills which scan up to depth 3)
2. Enumerate all `*.md` files directly within `agents/` (not nested subdirectories)
3. Each `.md` file is one agent, identified by filename without extension

This runs alongside skill discovery (which looks for `SKILL.md` recursively up to depth 3). The key difference: skills are directories containing `SKILL.md`, agents are individual `.md` files in a flat `agents/` directory.

### 5.3 Data Structures

```rust
pub struct AgentInfo {
    pub name: String,                         // filename without .md extension
    pub source_path: PathBuf,                 // absolute path to .md file in source
    pub install_status: SkillInstallStatus,   // reuse existing enum
}
```

`SourceGroup` gains a new field:
```rust
pub struct SourceGroup {
    pub name: String,
    pub kind: SourceKind,
    pub path: PathBuf,
    pub skills: Vec<SkillInfo>,
    pub agents: Vec<AgentInfo>,  // NEW
}
```

### 5.4 Install / Uninstall

- **Install:** Create symlink `agents_source/{name}.md` → `source_path`
- **Uninstall:** Remove symlink from `agents_source/`
- **Conflict:** Same name from different source → `SkillInstallStatus::Conflict`

### 5.5 Link Flow

`agm link` creates, for each installed tool:
1. Prompt symlink (unchanged)
2. Skills dir symlink (unchanged)
3. **Agents dir symlink** (NEW): `{config_dir}/{agents_dir}` → `agents_source`

Skip agents link if `agents_dir` is empty string.

## 6. Command Restructuring: `agm skills` → `agm source`

### 6.1 New CLI Interface

```
agm source              # No args → enter interactive TUI
agm source -a <url>     # --add: clone repo, install skills & agents
agm source -u           # --update: git pull all repos
agm source -l           # --list: text-based listing of all sources
```

### 6.2 Clap Definition

```rust
#[derive(Args)]
pub struct SourceArgs {
    /// Add a source repository by URL or local path
    #[arg(short = 'a', long = "add")]
    pub add: Option<String>,

    /// Update all source repositories (git pull)
    #[arg(short = 'u', long = "update")]
    pub update: bool,

    /// List all sources and their skills/agents
    #[arg(short = 'l', long = "list")]
    pub list: bool,
}
```

### 6.3 Behavior

- No args and no flags → launch TUI
- `-a <url>` → clone/pull repo, scan for skills and agents, install interactively or with `--all`
- `-u` → `update_all()` then exit
- `-l` → print text listing (source groups with skill/agent counts and install status)

### 6.4 Add Flow Enhancement

`-a` now also discovers and installs agents from the added source, not just skills.

## 7. TUI Overhaul

### 7.1 Three-Level Hierarchy

```
Level 1: Category headers     (Skills, Agents)
Level 2: Source groups         (superpowers, gstack, myskills, ...)
Level 3: Individual items      (skill dirs, agent .md files)
```

### 7.2 Layout

```
┌───────────────────────────────────────────────┐
│ ▼ Skills                                      │
│   ▶ superpowers (12 skills)            [OFF]  │
│   ▼ gstack (8 skills)                  [ON]   │
│     ✓ brainstorming                           │
│     ✗ code-auditor                            │
│     ✓ debugging-techniques                    │
│   ▶ myskills (3 skills)                [OFF]  │
│ ▼ Agents                                      │
│   ▶ superpowers (3 agents)             [OFF]  │
│   ▼ myskills (2 agents)                [ON]   │
│     ✓ code-reviewer                           │
│     ✗ design-critic                           │
├───────────────────────────────────────────────┤
│ [Space/Enter] Toggle  [a] Add  [u] Update     │
│ [0] Collapse All  [9] Expand All               │
│ [/] Search  [d] Delete  [q] Quit               │
└───────────────────────────────────────────────┘
```

### 7.3 Auto-Update on Launch

When TUI starts:
1. Show "Updating sources..." with progress
2. Run `update_all()` (git pull each repo)
3. Re-scan all sources
4. Transition to main interactive view

### 7.4 Toggle Behavior

| Context | Space/Enter Action |
|---------|-------------------|
| Level 1 (Skills/Agents header) | Expand/collapse all source groups under this category |
| Level 2 (Source group) | Expand/collapse the skill/agent list within this source |
| Level 3 (Individual skill/agent) | Install/uninstall (toggle symlink) |

**Collapse/Expand shortcuts:**
- `0` — collapse all levels (both categories, all source groups)
- `9` — expand all levels

**Default state:** All source groups collapsed (OFF).

### 7.5 Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `↓` | Move cursor down |
| `k` / `↑` | Move cursor up |
| `Space` / `Enter` | Toggle (context-dependent, see 7.4) |
| `0` | Collapse all |
| `9` | Expand all |
| `a` | Add source (prompt for URL in-TUI) |
| `u` | Re-run update |
| `d` | Delete source (with confirmation) |
| `/` | Enter search mode |
| `Esc` | Exit search / cancel |
| `q` | Quit TUI |

### 7.6 Search Mode

Activated by `/`:
1. Show search input field at bottom
2. **Fuzzy matching** on skill/agent names (use substring or fuzzy algorithm)
3. **Dynamic filtering:** only show items matching the query
   - Automatically expand source groups containing matches
   - Hide non-matching items and empty source groups
   - Category headers (Skills/Agents) remain visible if they contain any matches
4. **Interactive while searching:** `Space`/`Enter` on Level 3 items still toggles install/uninstall
5. `Esc` exits search, restores full unfiltered view
6. Search query updates on each keystroke (real-time filtering)

## 8. File Changes Summary

| File | Action | Changes |
|------|--------|---------|
| `src/config.rs` | Modify | Remove `files_base`/`files`, add `agents_source`/`agents_dir`, rename `skill_repos`→`source_repos` |
| `src/files.rs` | Delete | Entire module removed |
| `src/init.rs` | Modify | Add agents dir creation, remove files dir, add 3 new default tools |
| `src/main.rs` | Modify | Rename `skills`→`source` command, change subcommands to flags, remove files from link/unlink, add agents link/unlink |
| `src/skills.rs` | Modify | Add agent discovery, AgentInfo struct, install/uninstall agents, update scan_all_sources |
| `src/linker.rs` | Modify | Remove file-specific logic (if any), keep generic link/unlink |
| `src/status.rs` | Modify | Remove files display, add agents display, show agents count |
| `src/manage.rs` | Modify | Major TUI rewrite: 3-level hierarchy, agents panel, fuzzy search, auto-update, keyboard shortcuts |
| `src/paths.rs` | No change | — |
| `src/editor.rs` | No change | — |
| `src/platform.rs` | No change | — |
| `Cargo.toml` | Possibly | Add fuzzy matching crate if needed (e.g., `fuzzy-matcher`) |
| `README.md` | Modify | Update commands, config schema, tool list |
| `CHANGELOG.md` | Modify | Add v0.6.0 entry |

## 9. Central Directory Layout (Post-Refactor)

```
~/.local/share/agm/
├── prompts/
│   └── MASTER.md
├── skills/                    # symlinks to skill directories
│   ├── brainstorming/ → ../../source/superpowers/skills/brainstorming/
│   └── ...
├── agents/                    # NEW: symlinks to agent .md files
│   ├── code-reviewer.md → ../../source/superpowers/agents/code-reviewer.md
│   └── ...
└── source/                    # git-cloned repos
    ├── superpowers/
    │   ├── skills/
    │   │   ├── brainstorming/
    │   │   │   └── SKILL.md
    │   │   └── ...
    │   └── agents/
    │       ├── code-reviewer.md
    │       └── ...
    ├── myskills/
    └── ...
```

## 10. Breaking Changes

1. **Config schema** — `skill_repos` renamed to `source_repos`; `files_base` and `files` removed from `[central]`; `files` removed from `[tools.*]`; `agents_source` added to `[central]`; `agents_dir` added to `[tools.*]`
2. **CLI command** — `agm skills` removed, replaced by `agm source` with flags
3. **Central directory** — `~/.local/share/agm/files/` no longer managed (left on disk, not deleted)
4. **No automatic migration** — users must manually update their `config.toml`

## 11. Testing Strategy

- Update existing unit tests in each module for renamed fields
- Add unit tests for agent discovery (`scan_agents()`)
- Add unit tests for agent install/uninstall
- Add integration tests for `agm source -l`, `agm source -u`
- TUI testing: manual verification (ratatui TUI is hard to unit test)
