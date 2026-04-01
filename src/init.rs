use colored::Colorize;
use std::fs;
use std::path::PathBuf;

use crate::config::Config;
use crate::paths::expand_tilde;

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

    // Load config to get central paths
    let config = Config::load_from(config_path_override.clone())?;

    // Create central directories
    let dirs_to_create = [
        &config.central.skills_source,
        &config.central.agents_source,
        &config.central.source_dir,
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
    let prompt_path = expand_tilde(&config.central.prompt_source);
    if prompt_path.exists() {
        println!(
            "{} {} already exists",
            "skip".yellow(),
            config.central.prompt_source
        );
    } else {
        if let Some(parent) = prompt_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&prompt_path, "# Shared AI Agent Prompt\n\n")?;
        println!(
            "{} Created {}",
            " ok ".green(),
            config.central.prompt_source
        );
    }

    // Detect installed tools
    println!("\n{}", "Detected tools:".bold());
    let config = Config::load_from(config_path_override)?;
    for (key, tool) in &config.tools {
        let status = if tool.is_installed() {
            "installed".green()
        } else {
            "not found".dimmed()
        };
        println!("  {} ({}) — {}", key, tool.name, status);
    }

    println!("\n{}", "Run `agm link` to create links.".dimmed());
    Ok(())
}
