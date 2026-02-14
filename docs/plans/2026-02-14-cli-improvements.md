# CLI Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add three CLI enhancements: lowercase `-v` for version, auto-help on missing args, and unified edit command with interactive tool selection.

**Architecture:** All changes in `src/main.rs`. Three independent features: (1) custom version flag handler, (2) clap error wrapper for help display, (3) restructured Edit command with helper functions for interactive selection and file opening.

**Tech Stack:** Rust, clap (derive macros), std::io for interactive input

---

## Task 1: Add Lowercase Version Flag Support

**Files:**
- Modify: `src/main.rs:12-17` (Cli struct)
- Modify: `src/main.rs:63-64` (main function)

**Step 1: Disable default version flag and add custom -v arg**

Edit the `Cli` struct in `src/main.rs`:

```rust
#[derive(Parser)]
#[command(name = "agm", about = "AI Agent Manager", disable_version_flag = true)]
struct Cli {
    /// Print version information
    #[arg(short = 'v', long = "version")]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}
```

**Step 2: Add version check at start of main()**

Modify `main()` function in `src/main.rs` (before the match statement):

```rust
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle version flag
    if cli.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Extract command (required if not showing version)
    let command = cli.command.expect("subcommand is required");

    match command {
        // ... rest of existing match arms
```

**Step 3: Update all match arms to use `command` variable**

Change all `Commands::` patterns from `cli.command` to `command`:

```rust
    match command {
        Commands::Init => init::run(),
        Commands::Status => status::status(),
        // ... rest unchanged
```

**Step 4: Test version flag**

```bash
# Build and test
cargo build
./target/debug/agm -v
./target/debug/agm --version
```

Expected output:
```
agm 0.1.0
```

**Step 5: Test version flag ignores subcommand**

```bash
./target/debug/agm -v status
```

Expected: Shows version, not status

**Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: support lowercase -v flag for version display

- Disable clap default version flag
- Add custom -v/--version argument
- Check version early in main() and exit

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 2: Show Full Help on Missing Arguments

**Files:**
- Modify: `src/main.rs:63-64` (main function start)

**Step 1: Replace Cli::parse() with try_parse() and error handler**

Modify the start of `main()` function:

```rust
fn main() -> anyhow::Result<()> {
    // Parse CLI with custom error handling
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            use clap::error::ErrorKind;
            match e.kind() {
                ErrorKind::MissingRequiredArgument
                | ErrorKind::InvalidSubcommand
                | ErrorKind::MissingSubcommand => {
                    // Show full help instead of brief error
                    let mut cmd = Cli::command();
                    cmd.print_help()?;
                    println!(); // Add newline after help
                    std::process::exit(1);
                }
                _ => {
                    // Keep default error handling for other errors
                    e.exit();
                }
            }
        }
    };

    // Handle version flag
    if cli.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Extract command (required if not showing version)
    let command = cli.command.expect("subcommand is required");

    match command {
        // ... rest unchanged
```

**Step 2: Test help on missing args**

```bash
cargo build
./target/debug/agm edit
./target/debug/agm unlink
./target/debug/agm skills add
```

Expected: Each should display full help text (same as `agm -h`)

**Step 3: Verify normal help still works**

```bash
./target/debug/agm -h
./target/debug/agm --help
```

Expected: Help displays and exits with code 0

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: display full help when required parameters missing

- Wrap Cli::parse() with try_parse() and error matching
- Show full help for missing args instead of brief error
- Preserve default behavior for other error types

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 3: Restructure Edit Command Arguments

**Files:**
- Modify: `src/main.rs:44-51` (Edit enum variant)
- Modify: `src/main.rs:184-237` (Edit command handler)

**Step 1: Update Edit command struct**

Modify the `Commands` enum in `src/main.rs`:

```rust
    /// Open config files in editor
    Edit {
        /// File type: "prompt", "config", "auth", "mcp"
        file_type: String,
        /// Optional tool name (claude, gemini, copilot, etc.)
        tool: Option<String>,
    },
```

**Step 2: Update Edit command match arm signature**

Change the match pattern from:

```rust
Commands::Edit { target, file_type } => {
```

to:

```rust
Commands::Edit { file_type, tool } => {
```

**Step 3: Stub out new match logic**

Replace the entire Edit match arm body with a placeholder:

```rust
Commands::Edit { file_type, tool } => {
    let config = config::Config::load()?;
    let ed = editor::get_editor(&config);

    // TODO: Implement new unified edit logic
    println!("Edit: file_type={}, tool={:?}", file_type, tool.as_deref().unwrap_or("none"));
    Ok(())
}
```

**Step 4: Test that it compiles**

```bash
cargo build
```

Expected: Compiles successfully

**Step 5: Test new argument structure**

```bash
./target/debug/agm edit prompt
./target/debug/agm edit config claude
```

Expected: Prints stub messages showing parsed arguments

**Step 6: Commit**

```bash
git add src/main.rs
git commit -m "refactor: restructure Edit command to file_type + optional tool

BREAKING CHANGE: Edit command syntax changed
- Old: agm edit <tool> <file_type>
- New: agm edit <file_type> [tool]

Implementation stubbed for now.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 4: Implement Helper Function for Tool File Opening

**Files:**
- Modify: `src/main.rs:240+` (add new function before main())

**Step 1: Add required imports**

Add to the top of `src/main.rs` after existing use statements:

```rust
use std::io::{self, Write};
use std::path::{Path, PathBuf};
```

**Step 2: Write open_tool_files helper function**

Add this function before `fn main()`:

```rust
fn open_tool_files(
    config: &config::Config,
    ed: &str,
    tool_name: &str,
    file_type: &str,
) -> anyhow::Result<()> {
    let tool_config = config.tools.get(tool_name)
        .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found in config", tool_name))?;

    let base_dir = tool_config.resolved_config_dir();

    let files_to_open: Vec<PathBuf> = match file_type {
        "prompt" => {
            if tool_config.prompt_filename.is_empty() {
                anyhow::bail!("Tool '{}' has no prompt file configured", tool_name);
            }
            vec![base_dir.join(&tool_config.prompt_filename)]
        }
        "config" => {
            tool_config.settings.iter()
                .map(|f| base_dir.join(f))
                .collect()
        }
        "auth" => {
            tool_config.auth.iter()
                .map(|f| base_dir.join(f))
                .collect()
        }
        "mcp" => {
            tool_config.mcp.iter()
                .map(|f| base_dir.join(f))
                .collect()
        }
        _ => unreachable!("Invalid file_type: {}", file_type),
    };

    if files_to_open.is_empty() {
        anyhow::bail!("No {} files configured for {}", file_type, tool_name);
    }

    let file_refs: Vec<&Path> = files_to_open.iter().map(|p| p.as_path()).collect();
    editor::open_files(ed, &file_refs)?;

    Ok(())
}
```

**Step 3: Test that it compiles**

```bash
cargo build
```

Expected: Compiles successfully (function not used yet)

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: add open_tool_files helper for edit command

Opens tool-specific files based on file_type.
Validates tool exists and has files configured.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 5: Implement Helper Function for Interactive Tool Selection

**Files:**
- Modify: `src/main.rs` (add new function before open_tool_files)

**Step 1: Write select_installed_tool helper function**

Add this function before `open_tool_files()`:

```rust
fn select_installed_tool(config: &config::Config) -> anyhow::Result<String> {
    let installed_tools: Vec<_> = config.tools.iter()
        .filter(|(_, tool)| tool.is_installed())
        .collect();

    if installed_tools.is_empty() {
        anyhow::bail!("No tools are installed");
    }

    println!("Select a tool:");
    for (i, (key, tool)) in installed_tools.iter().enumerate() {
        println!("  {}) {} ({})", i + 1, key, tool.name);
    }

    print!("\nEnter number (or q to quit): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input == "q" || input == "Q" {
        std::process::exit(0);
    }

    let index: usize = input.parse()
        .map_err(|_| anyhow::anyhow!("Invalid input: please enter a number"))?;

    installed_tools.get(index.saturating_sub(1))
        .map(|(key, _)| key.to_string())
        .ok_or_else(|| anyhow::anyhow!("Invalid selection: number out of range"))
}
```

**Step 2: Test that it compiles**

```bash
cargo build
```

Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add select_installed_tool for interactive menu

Displays numbered list of installed tools.
Supports quit with 'q' and validates numeric input.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 6: Implement Unified Edit Command Logic

**Files:**
- Modify: `src/main.rs` (Edit match arm)

**Step 1: Replace stub with full implementation**

Replace the Edit match arm body in `fn main()`:

```rust
Commands::Edit { file_type, tool } => {
    let config = config::Config::load()?;
    let ed = editor::get_editor(&config);

    match (file_type.as_str(), tool) {
        // No tool specified - open master files or show selection
        ("prompt", None) => {
            let prompt_path = paths::expand_tilde(&config.central.prompt_source);
            editor::open_files(&ed, &[&prompt_path])?;
        }
        ("config", None) => {
            let config_path = config::Config::config_path();
            editor::open_files(&ed, &[&config_path])?;
        }
        ("auth", None) | ("mcp", None) => {
            let selected_tool = select_installed_tool(&config)?;
            open_tool_files(&config, &ed, &selected_tool, file_type.as_str())?;
        }

        // Tool specified - open tool-specific files
        ("prompt", Some(tool_name))
        | ("config", Some(tool_name))
        | ("auth", Some(tool_name))
        | ("mcp", Some(tool_name)) => {
            open_tool_files(&config, &ed, tool_name, file_type.as_str())?;
        }

        // Invalid file type
        (invalid, _) => {
            anyhow::bail!(
                "Invalid file type: '{}'. Use: prompt, config, auth, or mcp",
                invalid
            );
        }
    }

    Ok(())
}
```

**Step 2: Build and test compilation**

```bash
cargo build
```

Expected: Compiles successfully

**Step 3: Test edit prompt (master)**

```bash
./target/debug/agm edit prompt
```

Expected: Opens master MASTER.md in editor

**Step 4: Test edit config (agm)**

```bash
./target/debug/agm edit config
```

Expected: Opens agm's config.toml in editor

**Step 5: Test edit with tool - prompt**

```bash
./target/debug/agm edit prompt claude
```

Expected: Opens ~/.claude/PROMPT.md (or configured path) in editor

**Step 6: Test interactive selection - auth**

```bash
./target/debug/agm edit auth
```

Expected: Shows interactive menu, opens selected tool's auth file

**Step 7: Test invalid file type**

```bash
./target/debug/agm edit invalid
```

Expected: Error message: "Invalid file type: 'invalid'. Use: prompt, config, auth, or mcp"

**Step 8: Test tool not found**

```bash
./target/debug/agm edit prompt nonexistent
```

Expected: Error message: "Tool 'nonexistent' not found in config"

**Step 9: Commit**

```bash
git add src/main.rs
git commit -m "feat: implement unified edit command logic

- Support prompt/config without tool (master files)
- Support auth/mcp without tool (interactive selection)
- Support all file types with tool parameter
- Validate file types and tool existence

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 7: Update Documentation - README

**Files:**
- Modify: `README.md:64-71` (Editing Shortcuts section)

**Step 1: Update Editing Shortcuts section**

Replace the "Editing Shortcuts" section in README.md:

```markdown
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

**Breaking Change (v0.2.0):** Command syntax changed from `agm edit <tool> <file_type>` to `agm edit <file_type> [tool]`.
```

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: update README with new unified edit command syntax

Add examples and note breaking change from v0.1.0.
Document interactive selection for auth/mcp.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 8: Update Documentation - CHANGELOG

**Files:**
- Create: `CHANGELOG.md` (if doesn't exist)
- Modify: `CHANGELOG.md` (add Unreleased section)

**Step 1: Check if CHANGELOG exists**

```bash
ls CHANGELOG.md
```

**Step 2: Read existing CHANGELOG or create new one**

If exists, read it. If not, create with standard header.

**Step 3: Add Unreleased section**

Add to top of CHANGELOG.md (after header):

```markdown
## [Unreleased]

### Added
- Support for lowercase `-v` flag to display version information
- Interactive tool selection menu for `agm edit auth` and `agm edit mcp` commands when tool not specified
- Full help text display when required command parameters are missing (instead of brief error)

### Changed
- **BREAKING**: Unified edit command syntax from `agm edit <tool> <file_type>` to `agm edit <file_type> [tool]`
  - Old: `agm edit claude config`
  - New: `agm edit config claude`
- Version flag now uses lowercase `-v` instead of uppercase `-V`

### Migration Guide

**Edit Command Syntax Change:**
- Before (v0.1.0): `agm edit <tool> <file_type>`
- After (v0.2.0): `agm edit <file_type> [tool]`

Examples:
- `agm edit claude config` → `agm edit config claude`
- `agm edit gemini auth` → `agm edit auth gemini`
- `agm edit prompt` → **unchanged** (opens MASTER.md)
- `agm edit config` → **unchanged** (opens agm config.toml)

```

**Step 4: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: add CHANGELOG entries for v0.2.0 features

Document three new features and breaking change.
Add migration guide for edit command syntax.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 9: Final Testing and Validation

**Files:**
- None (testing only)

**Step 1: Test all version flag scenarios**

```bash
cargo build --release
./target/release/agm -v
./target/release/agm --version
./target/release/agm -v status
```

Expected:
- All show: `agm 0.1.0` (or current version)
- Third command ignores `status` subcommand

**Step 2: Test help on missing args**

```bash
./target/release/agm edit
./target/release/agm unlink
./target/release/agm skills add
```

Expected: Each displays full help text

**Step 3: Test unified edit - no tool**

```bash
./target/release/agm edit prompt
./target/release/agm edit config
```

Expected:
- First opens MASTER.md
- Second opens config.toml

**Step 4: Test unified edit - with tool**

```bash
./target/release/agm edit prompt claude
./target/release/agm edit config claude
./target/release/agm edit auth claude
```

Expected: Opens respective tool files

**Step 5: Test interactive selection**

```bash
./target/release/agm edit auth
# Enter: 1
./target/release/agm edit mcp
# Enter: q
```

Expected:
- First opens auth files for selected tool
- Second quits cleanly

**Step 6: Test error cases**

```bash
./target/release/agm edit invalid
./target/release/agm edit prompt nonexistent
./target/release/agm edit auth
# Enter: 999
```

Expected: Appropriate error messages for each

**Step 7: Verify breaking change**

```bash
./target/release/agm edit claude config
```

Expected: Should fail (show help or error) - old format no longer works

**Step 8: Create final verification commit**

```bash
git add -A
git commit -m "test: verify all v0.2.0 features working

Tested:
- Version flag with -v
- Help display on missing args
- Unified edit command (all scenarios)
- Interactive tool selection
- Error handling

All tests passing.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Success Criteria

- [x] `agm -v` shows version and exits
- [x] `agm --version` shows version
- [x] `agm -v <subcommand>` shows version (ignores subcommand)
- [x] Missing required args show full help instead of error
- [x] `agm edit prompt` opens MASTER.md
- [x] `agm edit config` opens agm config.toml
- [x] `agm edit auth` shows interactive tool selection
- [x] `agm edit mcp` shows interactive tool selection
- [x] `agm edit <file_type> <tool>` opens correct tool files
- [x] Interactive selection accepts number and 'q'
- [x] Invalid file types show error with valid options
- [x] Non-existent tools show error
- [x] Invalid selection shows error
- [x] README.md updated with new syntax and examples
- [x] CHANGELOG.md has Unreleased section with breaking change noted
- [x] Old command format (`agm edit <tool> <file_type>`) no longer works

## Notes

- All changes are in `src/main.rs` - no other modules modified
- Two new helper functions added: `select_installed_tool()` and `open_tool_files()`
- New imports required: `std::io::{self, Write}` and `std::path::{Path, PathBuf}`
- Breaking change requires version bump to 0.2.0
- TDD not applicable here (CLI testing is manual/integration)
- Frequent small commits maintained throughout
