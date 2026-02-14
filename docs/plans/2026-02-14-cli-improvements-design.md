# AGM CLI Improvements Design

**Date:** 2026-02-14
**Status:** Approved
**Version:** v0.2.0

## Overview

Three enhancements to AGM's CLI interface:
1. Support lowercase `-v` flag for version (instead of `-V`)
2. Display full help text when required parameters are missing
3. Unified edit command structure with interactive tool selection

## Requirements

### 1. Version Flag: Lowercase `-v`
- Current: clap's default uses `-V/--version`
- Desired: `-v/--version` (lowercase only)

### 2. Help on Missing Arguments
- Current: Shows clap error message when parameters missing
- Desired: Display full help text (same as `-h`) for all commands

### 3. Unified Edit Command Structure
- Current: `agm edit <target> [file_type]` where target can be "prompt", "config", or tool name
- Desired: `agm edit [FILE_TYPE] [TOOL]` where FILE_TYPE is "prompt", "config", "auth", or "mcp"

**New behavior matrix:**

| Command | Behavior |
|---------|----------|
| `agm edit prompt` | Open master MASTER.md |
| `agm edit prompt <tool>` | Open tool's prompt file |
| `agm edit config` | Open agm's config.toml |
| `agm edit config <tool>` | Open tool's settings files |
| `agm edit auth` | Interactive tool selection menu |
| `agm edit auth <tool>` | Open tool's auth files |
| `agm edit mcp` | Interactive tool selection menu |
| `agm edit mcp <tool>` | Open tool's mcp config |

## Architecture

All changes contained in `main.rs`. No modifications to other modules.

### Component Breakdown

**1. Version Flag Handler**
- Disable clap's default version flag
- Add custom `-v` arg at Cli struct level
- Check flag early in main(), print version and exit

**2. Help Display Handler**
- Wrap `Cli::parse()` with error handling
- Match on clap `ErrorKind`
- Convert missing argument errors to help display

**3. Unified Edit Command**
- Restructure `Edit` enum variant with `file_type` and optional `tool`
- Pattern match on `(file_type, tool)` tuple
- Add interactive tool selection function for auth/mcp without tool
- Extract file opening logic to helper function

## Implementation Details

### Version Flag

**Cli struct changes:**
```rust
#[derive(Parser)]
#[command(name = "agm", about = "AI Agent Manager", disable_version_flag = true)]
struct Cli {
    #[arg(short = 'v', long = "version", help = "Print version")]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,  // Optional since -v doesn't need command
}
```

**Main function check:**
```rust
let cli = Cli::parse();

if cli.version {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    return Ok(());
}

let command = cli.command.expect("command is required");
```

### Help on Missing Args

**Error handler wrapper:**
```rust
let cli = match Cli::try_parse() {
    Ok(cli) => cli,
    Err(e) => {
        use clap::error::ErrorKind;
        match e.kind() {
            ErrorKind::MissingRequiredArgument
            | ErrorKind::InvalidSubcommand
            | ErrorKind::MissingSubcommand => {
                let mut cmd = Cli::command();
                cmd.print_help()?;
                std::process::exit(1);
            }
            _ => e.exit(),
        }
    }
};
```

### Unified Edit Command

**Edit enum variant:**
```rust
Edit {
    /// File type: "prompt", "config", "auth", "mcp"
    file_type: String,
    /// Optional tool name (claude, gemini, copilot, etc.)
    tool: Option<String>,
}
```

**Match logic:**
```rust
Commands::Edit { file_type, tool } => {
    let config = config::Config::load()?;
    let ed = editor::get_editor(&config);

    match (file_type.as_str(), tool) {
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
        ("prompt", Some(tool_name)) | ("config", Some(tool_name))
        | ("auth", Some(tool_name)) | ("mcp", Some(tool_name)) => {
            open_tool_files(&config, &ed, tool_name, file_type.as_str())?;
        }
        (invalid, _) => {
            anyhow::bail!("Invalid file type: {}. Use: prompt, config, auth, or mcp", invalid);
        }
    }
    Ok(())
}
```

**Helper function: Interactive tool selection**
```rust
fn select_installed_tool(config: &Config) -> anyhow::Result<String> {
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
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input == "q" {
        std::process::exit(0);
    }

    let index: usize = input.parse()
        .map_err(|_| anyhow::anyhow!("Invalid input"))?;

    installed_tools.get(index - 1)
        .map(|(key, _)| key.to_string())
        .ok_or_else(|| anyhow::anyhow!("Invalid selection"))
}
```

**Helper function: Open tool files**
```rust
fn open_tool_files(
    config: &Config,
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
        _ => unreachable!(),
    };

    if files_to_open.is_empty() {
        anyhow::bail!("No {} files configured for {}", file_type, tool_name);
    }

    let file_refs: Vec<&Path> = files_to_open.iter().map(|p| p.as_path()).collect();
    editor::open_files(ed, &file_refs)?;

    Ok(())
}
```

**Required imports:**
```rust
use std::io::{self, Write};
use std::path::{Path, PathBuf};
```

## Error Handling

### Version Flag
- **Edge case**: `agm -v status` → show version and exit (ignore subcommand)
- **Solution**: Check version flag before command processing

### Help Display
- **Coverage**: Handle `MissingRequiredArgument`, `InvalidSubcommand`, `MissingSubcommand`
- **Preserve**: Other error types (e.g., `--help`) keep default behavior
- **Exit code**: exit(1) after showing help for errors, vs exit(0) for `--help`

### Edit Command
- **Invalid file type**: Error message with valid options
- **Tool not found**: Clear error with tool name
- **No files configured**: Specify which file type is missing for which tool
- **No tools installed**: When interactive selection invoked but no tools available
- **Invalid selection input**: Non-numeric or out-of-range input
- **Quit selection**: 'q' input exits cleanly with code 0
- **Empty prompt_filename**: Check and error before attempting to open

## Breaking Changes

**BREAKING CHANGE: Edit command syntax**

- **Old format**: `agm edit <tool> <file_type>`
- **New format**: `agm edit <file_type> [tool]`

**Impact:**
- Any scripts, aliases, or documentation using old format must be updated
- Old commands will fail with help display (due to parameter mismatch)

**Migration:**
- `agm edit claude config` → `agm edit config claude`
- `agm edit prompt` → **unchanged** (opens MASTER.md)
- `agm edit config` → **unchanged** (opens agm config)

## Testing Plan

### Version Flag Tests
```bash
agm -v                    # ✓ Show version
agm --version             # ✓ Show version
agm -v status             # ✓ Show version, ignore subcommand
```

### Help Display Tests
```bash
agm edit                  # ✓ Show full help
agm unlink                # ✓ Show full help
agm skills add            # ✓ Show full help
```

### Edit Command Tests - No Tool
```bash
agm edit prompt           # ✓ Open MASTER.md
agm edit config           # ✓ Open agm config.toml
agm edit auth             # ✓ Interactive tool selection
agm edit mcp              # ✓ Interactive tool selection
```

### Edit Command Tests - With Tool
```bash
agm edit prompt claude    # ✓ Open ~/.claude/PROMPT.md
agm edit config claude    # ✓ Open claude's settings files
agm edit auth claude      # ✓ Open claude's auth files
agm edit mcp gemini       # ✓ Open gemini's mcp config
```

### Error Case Tests
```bash
agm edit unknown          # ✓ Error: invalid file type
agm edit prompt invalid   # ✓ Error: tool not found
agm edit auth             # (input 'q') ✓ Exit cleanly
agm edit auth             # (input '999') ✓ Error: invalid selection
```

### Breaking Change Verification
```bash
agm edit claude config    # ✓ Should fail (old format)
```

## Documentation Updates

### README.md
- Update "Editing Shortcuts" section with new unified format
- Add examples for interactive selection
- Note breaking change from v0.1.0

### CHANGELOG.md
Add to Unreleased section:
```markdown
## [Unreleased]

### Added
- Interactive tool selection for `agm edit auth` and `agm edit mcp` commands
- Support for lowercase `-v` flag to display version

### Changed
- **BREAKING**: Unified edit command syntax from `agm edit <tool> <file_type>` to `agm edit <file_type> [tool]`
- Display full help text when required parameters are missing (instead of brief error)

### Migration Guide
Edit command syntax has changed:
- Old: `agm edit <tool> <file_type>` (e.g., `agm edit claude config`)
- New: `agm edit <file_type> [tool]` (e.g., `agm edit config claude`)
```

## Implementation Order

1. Version flag support (simplest, isolated)
2. Help on missing args (error handling layer)
3. Unified edit command (most complex, requires helper functions)
4. Documentation updates
5. Manual testing verification

## Success Criteria

- [ ] `agm -v` shows version and exits
- [ ] Missing required args show full help instead of error
- [ ] `agm edit prompt` opens MASTER.md
- [ ] `agm edit config` opens agm config
- [ ] `agm edit auth` shows interactive selection
- [ ] `agm edit <file_type> <tool>` opens correct tool files
- [ ] Invalid inputs show appropriate error messages
- [ ] README and CHANGELOG updated
- [ ] All test cases pass
