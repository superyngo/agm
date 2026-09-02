# AGM (AI Agent Manager)

A Rust CLI tool for centralized management of AI coding agent CLI tools (Claude Code, Antigravity CLI, Copilot CLI, Codex CLI, Pi, Crush, OpenCode, etc.).

## Features

- **Centralized Configuration**: Manage prompts, skills, agents, and configs for all AI CLI tools in one place
- **Symlink Management**: Automatically create and maintain links from each tool to central sources (symlinks on Unix, junctions + hardlinks on Windows)
- **Skills & Agents Management**: Install skills (directory-based) and agents (single `.md` files) from local paths or git repos, with auto-update support
- **Interactive TUI**: Browse, search, and toggle skills/agents with a ratatui-based terminal UI — Tool Manager and Source Manager live in one shell, switched with `Tab`
- **Registry-Driven**: Add new tools by editing TOML config—no code changes needed
- **Status Monitoring**: Check link health and tool installation status at a glance

## Installation

### With Wenget

```bash
wenget install agm
```

### Quick Install (One-Line Command)

#### Linux / macOS (Bash)

```bash
curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.sh | APP_NAME="agm" REPO="YOUR_USERNAME/agm" bash
```

**Uninstall:**
```bash
curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.sh | APP_NAME="agm" REPO="YOUR_USERNAME/agm" bash -s uninstall
```

The installation script will:
- Automatically detect your OS and architecture
- Download the latest precompiled binary from GitHub Releases
- Install to `~/.local/bin`
- Add the installation directory to your PATH (if needed)

**Supported Platforms:**
- Linux (x86_64, i686, aarch64, armv7) - both GNU and musl
- macOS (x86_64, Apple Silicon)
- Windows (x86_64, i686)

---

### Manual Installation

#### From Precompiled Binaries

Download the latest release for your platform from the [Releases](https://github.com/YOUR_USERNAME/agm/releases) page.

**Linux/macOS:**
```bash
# Extract the downloaded tar.gz file and move agm to a directory in your PATH
tar -xzf agm-*.tar.gz
chmod +x agm
mv agm ~/.local/bin/
```

**Windows:**
```powershell
# Extract the downloaded zip file and move agm.exe to a directory in your PATH
Expand-Archive agm-windows-*.zip -DestinationPath .
Move-Item agm.exe "$env:USERPROFILE\.local\bin\"
```

---

#### From Source

If you prefer to build from source, ensure you have [Rust](https://rustup.rs/) installed:

```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/agm.git
cd agm

# Build release binary
cargo build --release

# The binary will be available at: target/release/agm

# Install manually
cp target/release/agm ~/.local/bin/
chmod +x ~/.local/bin/agm
```

## Quick Start

```bash
# Initialize config and central directories
agm init

# Open the config file in your editor
agm config

# Show status of all tools
agm tool status

# Create links for all installed tools
agm tool link

# Add a source from a git repo
agm source add https://github.com/anthropics/claude-code-skills

# Open the unified shell (Tab switches between Tool Manager and Source Manager)
agm

# Open interactive TUI to manage skills & agents
agm source

# Open interactive TUI to manage tools, links, and config
agm tool

# List all sources
agm source list

# Update all source repos
agm source update
```

## Commands

### Config

- `agm config` - Open the config file (`~/.config/agm/config.toml`) in `$EDITOR`, or the platform default if unset

### Tool Management

- `agm tool` - Open interactive TUI to manage tools, links, and configuration
- `agm tool status` - Show link status for all tools
- `agm tool link` - Create/repair all links (prompts + skills + agents)
- `agm tool unlink` - Remove all links

The TUI provides:
- View and toggle link status for each tool (prompt, skills, agents)
- Edit prompt, settings, auth, and MCP files with `e` key
- Edit central paths (skills, agents, source) inline
- File picker popup for multi-file fields
- Log popup (`o` key) for operation history

### TUI Keybindings

`agm` (bare), `agm tool`, and `agm source` all open the same unified shell — `agm tool` /
`agm source` just focus the matching screen on start. `Tab` / `Shift+Tab` switches between the
**Tool Manager** and **Source Manager** screens (only when no popup/input is active); the title
bar shows both with the active one bracketed, e.g. `agm — [Tool] · Source`.

Both screens share the same global keys:

| Key | Action |
| --- | --- |
| `↑`/`↓` (`k`/`j`) | Move cursor |
| `PgUp`/`PgDn` | Scroll by page |
| `Home`/`End` | Jump to top / bottom |
| `␣` / `⏎` | Toggle fold / open info |
| `9` / `0` | Expand / collapse all |
| `i` | Info popup for the row under the cursor |
| `o` | Open the log popup |
| `Tab` / `Shift+Tab` | Switch between Tool Manager and Source Manager |
| `?` | Open the **Help / About** panel — title shows `▸ Help  ·  About`; `Tab` switches tabs, `?`/`Esc` closes |
| `Esc` | Close popup / clear status (one layer) — does NOT quit |
| `q` / `Ctrl+C` | Quit |

In the **Source Manager**, additional keys: `s` toggle-select, `Shift+↑/↓` range-select, `Ctrl+A` select-all-in-source, `l` install/uninstall (batch when items are selected), `e` edit, `d` delete source, `a` add, `r` rename, `u` update, `F5` refresh, `/` fuzzy search. Selection is preserved across refresh/add/delete/rename.

In the **Tool Manager**, additional keys: `e` edit file or path, `l` toggle link / feature.

### Environment

| Variable | Effect |
| --- | --- |
| `NO_COLOR` | When set (to any value, including empty), both TUIs and the CLI strip ANSI color codes. Bold/italic modifiers are preserved so the focus cursor stays visible. |
| `EDITOR` | Used by `agm config` and the TUIs' edit actions; falls back to the platform default. |

### Source Management

- `agm source` - Open interactive TUI to manage skills & agents. In the item list, press `s` to toggle-select an item, `Shift+↑/↓` to range-select, then `l` to install/uninstall all selected items at once; `Esc` clears the selection.
- `agm source add <url>` - Add a source repo by URL
- `agm source update` - Update all source repos
- `agm source list` - List all sources with skills & agents
- `agm source del <name|url>` - Delete a source
- `agm source rename <old> <new>` - Rename a source folder and relink installed items
- `agm source add -n,--name <name> <url>` - Override the cloned/copied directory name

**Examples:**
```bash
# Non-interactive operations
agm tool status           # Show link status table
agm tool link             # Link all installed tools
agm tool unlink           # Remove all links

# Source operations
agm source add https://github.com/user/skills   # Add a source
agm source add -n my-skills https://github.com/user/skills  # Add with custom name
agm source del my-skills   # Delete a source by name
agm source rename old-name new-name  # Rename a source

# Interactive TUIs
agm tool                  # Tool management TUI
agm source                # Source management TUI
```

## Configuration

Config location: `~/.config/agm/config.toml`

Default central directories:
- Prompts: `~/.local/share/agm/prompts/MASTER.md`
- Skills: `~/.local/share/agm/skills/`
- Agents: `~/.local/share/agm/agents/`
- Source repos: `~/.local/share/agm/source/`

See [`CONTEXT.md`](CONTEXT.md) for the documentation index, or
[`docs/reference/config.md`](docs/reference/config.md) for the full config schema and
[`docs/reference/architecture.md`](docs/reference/architecture.md) for the architecture.

## Supported Tools

Out of the box support for 7 tools:
- Claude Code (`~/.claude`)
- Codex CLI (`~/.codex`)
- Copilot CLI (`~/.copilot`)
- Crush (`~/.config/crush`)
- Antigravity CLI (`~/.gemini`)
- OpenCode (`~/.config/opencode`)
- Pi (`~/.pi/agent`)

Add more tools by editing `config.toml` - no code changes needed!

## Development

```bash
# Run tests
cargo test

# Build debug
cargo build

# Build release
cargo build --release
```

## License

See LICENSE file.
