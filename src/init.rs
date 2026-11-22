use colored::Colorize;
use dialoguer::Confirm;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::config::Config;
use crate::paths::expand_tilde;
use crate::platform;

pub fn run(config_path_override: Option<PathBuf>) -> anyhow::Result<()> {
    let config_path = config_path_override
        .clone()
        .unwrap_or_else(Config::config_path);

    // Create config if not exists
    if config_path.exists() {
        println!(
            "{} Config already exists at {}",
            "skip".yellow(),
            config_path.display()
        );
    } else {
        let config = Config::default_config();
        config.save_to(&config_path)?;
        println!(
            "{} Created config at {}",
            " ok ".green(),
            config_path.display()
        );
    }

    // Load config to get agm paths
    let config = Config::load_from(config_path_override)?;

    // Create agm directories
    let dirs_to_create = [
        &config.agm.skills_source,
        &config.agm.agents_source,
        &config.agm.commands_source,
        &config.agm.source_dir,
    ];
    for dir in dirs_to_create {
        let path = expand_tilde(dir);
        if path.is_dir() {
            println!("{} {} already exists", "skip".yellow(), dir);
        } else {
            fs::create_dir_all(&path)?;
            println!("{} Created {}", " ok ".green(), dir);
        }
    }

    // Create prompt source parent dir and empty MASTER.md if not exists
    let prompt_path = expand_tilde(&config.agm.prompt_source);
    if prompt_path.exists() {
        println!(
            "{} {} already exists",
            "skip".yellow(),
            config.agm.prompt_source
        );
    } else {
        if let Some(parent) = prompt_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&prompt_path, "# Shared AI Agent Prompt\n\n")?;
        println!("{} Created {}", " ok ".green(), config.agm.prompt_source);
    }

    // Detect installed tools
    println!("\n{}", "Detected tools:".bold());
    for (key, tool) in &config.tools {
        let status = if tool.is_installed() {
            "installed".green()
        } else {
            "not found".dimmed()
        };
        println!("  {} ({}) — {}", key, tool.name, status);
    }

    print_dependency_report(&check_dependencies());

    println!("\n{}", "Run `agm tool link` to create links.".dimmed());
    Ok(())
}

/// Load the config, offering to run `agm init` when it is missing.
///
/// In a non-interactive context (no TTY, e.g. scripts or CI) this falls back to the
/// original hard error instead of prompting.
pub fn ensure_config(config_path_override: Option<PathBuf>) -> anyhow::Result<Config> {
    let config_path = config_path_override
        .clone()
        .unwrap_or_else(Config::config_path);

    if config_path.exists() {
        return Config::load_from(config_path_override);
    }

    println!(
        "{} Config not found at {}.",
        "warn".yellow(),
        config_path.display()
    );
    let run_init = Confirm::new()
        .with_prompt("Run `agm init` now?")
        .default(true)
        .interact()
        .unwrap_or(false);

    if !run_init {
        anyhow::bail!(
            "Config not found at {}. Run `agm init` first.",
            config_path.display()
        );
    }

    run(config_path_override.clone())?;
    Config::load_from(config_path_override)
}

/// One dependency probe result.
pub struct DependencyCheck {
    pub name: &'static str,
    pub ok: bool,
    pub hint: String,
}

/// Probe for runtime dependencies AGM relies on: `git` (used by `agm source add`/`update`)
/// and, on Windows, the Visual C++ Redistributable several managed Tools' native modules need.
pub fn check_dependencies() -> Vec<DependencyCheck> {
    let git_ok = Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    vec![
        DependencyCheck {
            name: "git",
            ok: git_ok,
            hint: platform::install_hint("git"),
        },
        DependencyCheck {
            name: "Microsoft Visual C++ Redistributable",
            ok: platform::vcredist_installed(),
            hint: platform::install_hint("vcredist"),
        },
    ]
}

/// Print one line per missing dependency with its install hint. Returns `true` when
/// everything AGM needs is present (nothing printed).
pub fn print_dependency_report(deps: &[DependencyCheck]) -> bool {
    let missing: Vec<&DependencyCheck> = deps.iter().filter(|d| !d.ok).collect();
    if missing.is_empty() {
        return true;
    }
    println!("\n{}", "Missing dependencies:".bold());
    for d in missing {
        println!("  {} {} — install: {}", "warn".red(), d.name, d.hint);
    }
    false
}
