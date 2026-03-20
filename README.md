# AGM (AI Agent Manager)

A Rust CLI tool for centralized management of AI coding agent CLI tools (Claude Code, Gemini CLI, Copilot CLI, OpenCode, etc.).

## Features

- **Centralized Configuration**: Manage prompts, skills, and configs for all AI CLI tools in one place
- **Symlink Management**: Automatically create and maintain links from each tool to central sources (symlinks on Unix, junctions + hardlinks on Windows)
- **Skills Management**: Install skills from local paths or git repos, with auto-update support
- **Registry-Driven**: Add new tools by editing TOML config—no code changes needed
- **Status Monitoring**: Check link health and tool installation status at a glance

## Installation

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

# Show status of all tools
agm status

# Create links for all installed tools
agm link

# Add skills from a git repo
agm skills add https://github.com/anthropics/claude-code-skills

# List installed skills
agm skills

# Update all skill repos
agm skills update
```

## Commands

### Status & Info

- `agm status` - Show link status for all tools

### Link Management

- `agm link` - Create/repair all links (prompts + skills)
- `agm link skills` - Only handle skills links
- `agm link prompts` - Only handle prompt links
- `agm unlink <tool>` - Remove links for a specific tool

### Skills Management

- `agm skills` - List installed skills with source paths
- `agm skills add <source>` - Install skill(s) from local path or repo URL
- `agm skills remove <name>` - Remove a skill link
- `agm skills update` - Git pull on all skill source repositories

### Editing Shortcuts

- `agm edit prompt` - Edit shared prompt master file (MASTER.md)
- `agm edit prompt <tool>` - Edit tool-specific prompt file
- `agm edit config` - Edit agm's own config.toml
- `agm edit config <tool>` - Open tool settings file(s)
- `agm edit auth` - Interactively select tool and open auth file(s)
- `agm edit auth <tool>` - Open tool auth file(s)
- `agm edit mcp` - Interactively select tool and open MCP config
- `agm edit mcp <tool>` - Open tool MCP config

**Examples:**
```bash
# Edit master files
agm edit prompt           # Opens shared MASTER.md
agm edit config           # Opens agm config.toml

# Edit tool-specific files
agm edit prompt claude    # Opens ~/.claude/PROMPT.md
agm edit config gemini    # Opens gemini settings
agm edit auth claude      # Opens claude auth files
agm edit mcp copilot      # Opens copilot MCP config

# Interactive selection
agm edit auth             # Shows menu to pick tool
agm edit mcp              # Shows menu to pick tool
```

**Breaking Change (v0.1.1):** Command syntax changed from `agm edit <tool> <file_type>` to `agm edit <file_type> [tool]`.

## Configuration

Config location: `~/.config/agm/config.toml`

Default central directories:
- Prompts: `~/.local/share/agm/prompts/MASTER.md`
- Skills: `~/.local/share/agm/skills/`
- Source repos: `~/.local/share/agm/source/`

See [design doc](docs/plans/2026-02-14-agm-design.md) for detailed architecture.

## Supported Tools

Out of the box support for:
- Claude Code (`~/.claude`)
- Gemini CLI (`~/.gemini`)
- Copilot CLI (`~/.copilot`)
- OpenCode (`~/.config/opencode`)

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
