use colored::Colorize;
use std::fs;

use crate::config::Config;
use crate::linker::{check_link, LinkStatus};
use crate::paths::{contract_tilde, expand_tilde};

/// Display table with tool name, config dir, prompt/skills link status
pub fn status() -> anyhow::Result<()> {
    let config = Config::load()?;
    let central_skills = expand_tilde(&config.central.skills_source);
    let central_prompt = expand_tilde(&config.central.prompt_source);

    println!("\n{}", "AGM — AI Agent Manager".bold());
    println!("{}", "═".repeat(62));
    println!(
        " {:<13} {:<23} {:<9} {:<9}",
        "Tool", "Config Dir", "Prompt", "Skills"
    );
    println!("{}", "─".repeat(62));

    for (key, tool) in &config.tools {
        if !tool.is_installed() {
            continue;
        }

        let prompt_status = if !tool.prompt_filename.is_empty() {
            let prompt_link = tool.resolved_config_dir().join(&tool.prompt_filename);
            match check_link(&prompt_link, &central_prompt) {
                LinkStatus::Linked => "✓ linked".green(),
                LinkStatus::Broken => "✗ broken".red(),
                LinkStatus::Wrong(_) => "✗ wrong".red(),
                LinkStatus::Blocked => "✗ blocked".red(),
                LinkStatus::Missing => "✗ missing".yellow(),
            }
        } else {
            "—".dimmed()
        };

        let skills_status = if !tool.skills_dir.is_empty() {
            let skills_link = tool.resolved_config_dir().join(&tool.skills_dir);
            match check_link(&skills_link, &central_skills) {
                LinkStatus::Linked => "✓ linked".green(),
                LinkStatus::Broken => "✗ broken".red(),
                LinkStatus::Wrong(_) => "✗ wrong".red(),
                LinkStatus::Blocked => "✗ blocked".red(),
                LinkStatus::Missing => "✗ missing".yellow(),
            }
        } else {
            "—".dimmed()
        };

        let config_dir = contract_tilde(&tool.resolved_config_dir());
        println!(
            " {:<13} {:<23} {:<9} {:<9}",
            format!("{} ({})", key, tool.name).dimmed(),
            config_dir.dimmed(),
            prompt_status,
            skills_status
        );
    }

    println!("{}", "═".repeat(62));

    // Count skills
    let skills_count = if central_skills.is_dir() {
        fs::read_dir(&central_skills)?.count()
    } else {
        0
    };

    println!("Central prompt : {}", contract_tilde(&central_prompt));
    println!(
        "Central skills : {} ({} skills)",
        contract_tilde(&central_skills),
        skills_count
    );
    println!();

    Ok(())
}

/// List registered tools with install status
pub fn list() -> anyhow::Result<()> {
    let config = Config::load()?;

    println!("\n{}", "Registered tools:".bold());
    for (key, tool) in &config.tools {
        let status = if tool.is_installed() {
            "installed".green()
        } else {
            "not found".dimmed()
        };
        let config_dir = contract_tilde(&tool.resolved_config_dir());
        println!("  {} ({}) — {} — {}", key, tool.name, config_dir, status);
    }
    println!();

    Ok(())
}

/// Verify all symlinks, report broken/wrong ones
pub fn check() -> anyhow::Result<()> {
    let config = Config::load()?;
    let central_skills = expand_tilde(&config.central.skills_source);
    let central_prompt = expand_tilde(&config.central.prompt_source);

    println!("\n{}", "Checking symlinks...".bold());

    let mut issues = 0;

    for (key, tool) in &config.tools {
        if !tool.is_installed() {
            continue;
        }

        // Check skills
        if !tool.skills_dir.is_empty() {
            let skills_link = tool.resolved_config_dir().join(&tool.skills_dir);
            match check_link(&skills_link, &central_skills) {
                LinkStatus::Linked => {}
                LinkStatus::Missing => {
                    println!("  {} {} skills: missing", "✗".red(), key);
                    issues += 1;
                }
                LinkStatus::Broken => {
                    println!("  {} {} skills: broken symlink", "✗".red(), key);
                    issues += 1;
                }
                LinkStatus::Wrong(target) => {
                    println!(
                        "  {} {} skills: points to {} (expected {})",
                        "✗".red(),
                        key,
                        target,
                        central_skills.display()
                    );
                    issues += 1;
                }
                LinkStatus::Blocked => {
                    println!("  {} {} skills: exists but not a symlink", "✗".red(), key);
                    issues += 1;
                }
            }
        }

        // Check prompt
        if !tool.prompt_filename.is_empty() {
            let prompt_link = tool.resolved_config_dir().join(&tool.prompt_filename);
            match check_link(&prompt_link, &central_prompt) {
                LinkStatus::Linked => {}
                LinkStatus::Missing => {
                    println!("  {} {} prompt: missing", "✗".red(), key);
                    issues += 1;
                }
                LinkStatus::Broken => {
                    println!("  {} {} prompt: broken symlink", "✗".red(), key);
                    issues += 1;
                }
                LinkStatus::Wrong(target) => {
                    println!(
                        "  {} {} prompt: points to {} (expected {})",
                        "✗".red(),
                        key,
                        target,
                        central_prompt.display()
                    );
                    issues += 1;
                }
                LinkStatus::Blocked => {
                    println!("  {} {} prompt: exists but not a symlink", "✗".red(), key);
                    issues += 1;
                }
            }
        }
    }

    if issues == 0 {
        println!("  {} All symlinks are healthy!", "✓".green());
    } else {
        println!("\n{} issues found. Run `agm link` to repair.", issues);
    }
    println!();

    Ok(())
}
