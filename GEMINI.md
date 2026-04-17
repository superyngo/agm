# AGM (AI Agent Manager) - Project Context

AGM is a Rust-based CLI tool designed to centralize and manage configurations for various AI coding agent CLI tools such as Claude Code, Gemini CLI, Copilot CLI, and others. It simplifies the management of prompts, skills, agents, and configurations by using a central store and symlinking (or junctions on Windows) to the respective tool configuration directories.

## Project Overview

- **Core Technology:** Rust (2021 edition)
- **Key Libraries:**
    - `clap`: Command-line argument parsing and subcommand management.
    - `ratatui` & `crossterm`: Interactive Terminal UI (TUI) components.
    - `serde` & `toml`: Configuration serialization and deserialization.
    - `anyhow`: Robust error handling.
    - `chrono`: Date and time formatting for backups and logs.
    - `dialoguer`: Interactive prompts for skill selection and confirmations.
- **Main Features:**
    - **Centralized Configuration:** Manage multiple AI tool settings from a single source of truth.
    - **Symlink Management:** Automatically create and maintain links from individual tools to the central store.
    - **Skills & Agents Management:** Install and update skills and agents from local paths or git repositories.
    - **Interactive TUIs:** Two main TUIs for managing tools (`agm tool`) and sources/skills (`agm source`).
    - **Platform Support:** Built-in support for Unix-like systems and Windows (using junctions/hardlinks).

## Architecture & Structure

- `src/main.rs`: The entry point that handles CLI argument parsing and dispatches to various modules.
- `src/config.rs`: Defines the configuration schema (`Config`, `CentralConfig`, `ToolConfig`) and handles loading/saving.
- `src/tui/`: Contains the TUI implementation using `ratatui`.
    - `mod.rs`: TUI event loop and common components.
    - `tool.rs`: Implementation of the tool management TUI.
    - `source.rs`: Implementation of the source and skill management TUI.
- `src/linker.rs`: Logic for creating, managing, and removing symlinks safely across platforms.
- `src/skills.rs`: Logic for scanning, cloning, and installing skills and agents from various sources.
- `src/init.rs`: Implements the `agm init` command to set up the default environment.
- `docs/`: Contains design documents, specifications, and project plans.
- `tests/`: Integration tests for the CLI commands using `assert_cmd`.

## Building and Running

### Development Commands
- **Build:** `cargo build`
- **Run (with args):** `cargo run -- <COMMAND>` (e.g., `cargo run -- tool`)
- **Test:** `cargo test` (runs both unit and integration tests)
- **Release Build:** `cargo build --release`

### Project Commands
- `agm init`: Initializes the configuration file and central directories.
- `agm tool`: Launches the interactive TUI for tool management.
- `agm tool --status`: Displays a table showing the current link status of all configured tools.
- `agm tool --link`: Non-interactively creates links for all installed tools.
- `agm source`: Launches the interactive TUI for managing skill sources.
- `agm source --add <URL/PATH>`: Adds a new source repository or local path for skills/agents.
- `agm source --update`: Updates all registered git-based sources.

## Development Conventions

- **Error Handling:** Use `anyhow::Result` for functions that can fail, providing clear error messages.
- **Configuration:** The default configuration is stored in `~/.config/agm/config.toml`. Use `agm init` to generate a fresh one.
- **Testing:**
    - Unit tests should be placed in the respective modules.
    - Integration tests are located in `tests/cli.rs` and verify CLI behavior using `assert_cmd`.
    - Always verify that changes don't break cross-platform link management (Unix symlinks vs. Windows junctions).
- **Styling:** Use the `colored` crate for meaningful terminal output outside of the TUI.
- **TUI Development:** Follow the `ratatui` patterns for state management and UI rendering found in `src/tui/`.

## Key Files
- `Cargo.toml`: Project dependencies and metadata.
- `src/config.rs`: The primary source for understanding how tools and central directories are structured.
- `src/linker.rs`: Essential for understanding the cross-platform symlinking strategy.
- `README.md`: High-level user documentation and installation instructions.
