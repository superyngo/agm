use colored::Colorize;
use std::fs;

use crate::config::Config;
use crate::files::{centralized_path, check_file_status, FileStatus};
use crate::linker::{check_link, LinkStatus};
use crate::paths::{contract_tilde, expand_path, expand_tilde};

/// Display table with tool name, config dir, prompt/skills link status and paths
pub fn status() -> anyhow::Result<()> {
    let config = Config::load()?;
    let central_skills = expand_tilde(&config.central.skills_source);
    let central_prompt = expand_tilde(&config.central.prompt_source);
    let files_base = expand_tilde(&config.central.files_base);

    // Indent for detail lines: aligns under the data columns
    const INDENT: &str = "                ";
    // Second-line indent for file central path: INDENT(16) + label(8) + status(9) + space(1)
    const FILE2: &str = "                          ";

    println!("\n{}", "AGM — AI Agent Manager".bold());
    println!("{}", "═".repeat(62));
    println!(" {:<13} {:<23}", "Tool", "Config Dir");
    println!("{}", "─".repeat(62));

    for (key, tool) in &config.tools {
        if !tool.is_installed() {
            continue;
        }

        let prompt_ls = if !tool.prompt_filename.is_empty() {
            let link = tool.resolved_config_dir().join(&tool.prompt_filename);
            Some(check_link(&link, &central_prompt))
        } else {
            None
        };

        let skills_ls = if !tool.skills_dir.is_empty() {
            let link = tool.resolved_config_dir().join(&tool.skills_dir);
            Some(check_link(&link, &central_skills))
        } else {
            None
        };

        let config_dir = contract_tilde(&tool.resolved_config_dir());
        println!(
            " {:<13} {:<23}",
            format!("{} ({})", key, tool.name).dimmed(),
            config_dir.dimmed(),
        );

        // Detail lines: prompt
        if let Some(ls) = prompt_ls {
            let prompt_link = tool.resolved_config_dir().join(&tool.prompt_filename);
            print!("{}{:<8}", INDENT, "prompt");
            match ls {
                LinkStatus::Linked => println!(
                    "{} → {}",
                    "✓ linked".green(),
                    contract_tilde(&prompt_link).dimmed()
                ),
                LinkStatus::Missing => println!(
                    "{} → {}",
                    "✗ missing".yellow(),
                    contract_tilde(&central_prompt).dimmed()
                ),
                LinkStatus::Broken => println!("{}", "✗ broken".red()),
                LinkStatus::Wrong(t) => println!("{} → {}", "✗ wrong".red(), t.dimmed()),
                LinkStatus::Blocked => println!("{}", "✗ blocked".red()),
            }
        }

        // Detail lines: skills
        if let Some(ls) = skills_ls {
            let skills_link = tool.resolved_config_dir().join(&tool.skills_dir);
            print!("{}{:<8}", INDENT, "skills");
            match ls {
                LinkStatus::Linked => println!(
                    "{} → {}",
                    "✓ linked".green(),
                    contract_tilde(&skills_link).dimmed()
                ),
                LinkStatus::Missing => println!(
                    "{} → {}",
                    "✗ missing".yellow(),
                    contract_tilde(&central_skills).dimmed()
                ),
                LinkStatus::Broken => println!("{}", "✗ broken".red()),
                LinkStatus::Wrong(t) => println!("{} → {}", "✗ wrong".red(), t.dimmed()),
                LinkStatus::Blocked => println!("{}", "✗ blocked".red()),
            }
        }

        // Detail lines: managed files
        for file_path in &tool.files {
            let original = tool.resolve_path(file_path);
            let central = centralized_path(&original, &files_base);
            let display = contract_tilde(&original);
            print!("{}{:<8}", INDENT, "file");
            match check_file_status(&original, &files_base) {
                FileStatus::Linked => {
                    println!("{} {}", "✓ linked".green(), display.dimmed());
                    println!("{}→ {}", FILE2, contract_tilde(&central).dimmed());
                }
                FileStatus::ReadyToLink => {
                    println!("{} {}", "→ ready ".cyan(), display.dimmed());
                    println!("{}→ {}", FILE2, contract_tilde(&central).dimmed());
                }
                FileStatus::Unmanaged => println!("{} {}", "  file  ".normal(), display.dimmed()),
                FileStatus::Missing => println!("{} {}", "✗ missing".yellow(), display.dimmed()),
                FileStatus::Broken => {
                    println!("{} {}", "✗ broken ".red(), display.dimmed());
                    println!("{}→ {}", FILE2, contract_tilde(&central).dimmed());
                }
                FileStatus::Wrong(t) => {
                    println!("{} {}", "✗ wrong ".red(), display.dimmed());
                    println!(
                        "{}→ {} (expected)",
                        FILE2,
                        contract_tilde(&central).dimmed()
                    );
                    println!("{}   was: {}", FILE2, t.dimmed());
                }
            }
        }
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
    let source_dir = expand_tilde(&config.central.source_dir);
    println!("Central source : {}", contract_tilde(&source_dir));
    println!("Central files  : {}", contract_tilde(&files_base));

    // Central-level managed files
    if !config.central.files.is_empty() {
        println!("{}", "─".repeat(62));
        println!(" {}", "Central managed files:".bold());
        for file_path in &config.central.files {
            let original = expand_path(file_path);
            let central = centralized_path(&original, &files_base);
            let display = contract_tilde(&original);
            print!("{}{:<8}", INDENT, "file");
            match check_file_status(&original, &files_base) {
                FileStatus::Linked => {
                    println!("{} {}", "✓ linked".green(), display.dimmed());
                    println!("{}→ {}", FILE2, contract_tilde(&central).dimmed());
                }
                FileStatus::ReadyToLink => {
                    println!("{} {}", "→ ready ".cyan(), display.dimmed());
                    println!("{}→ {}", FILE2, contract_tilde(&central).dimmed());
                }
                FileStatus::Unmanaged => println!("{} {}", "  file  ".normal(), display.dimmed()),
                FileStatus::Missing => println!("{} {}", "✗ missing".yellow(), display.dimmed()),
                FileStatus::Broken => {
                    println!("{} {}", "✗ broken ".red(), display.dimmed());
                    println!("{}→ {}", FILE2, contract_tilde(&central).dimmed());
                }
                FileStatus::Wrong(t) => {
                    println!("{} {}", "✗ wrong ".red(), display.dimmed());
                    println!(
                        "{}→ {} (expected)",
                        FILE2,
                        contract_tilde(&central).dimmed()
                    );
                    println!("{}   was: {}", FILE2, t.dimmed());
                }
            }
        }
    }
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
    let files_base = expand_tilde(&config.central.files_base);

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

        // Check managed files
        for file_path in &tool.files {
            let original = tool.resolve_path(file_path);
            match check_file_status(&original, &files_base) {
                FileStatus::Linked => {}
                FileStatus::Missing => {
                    println!("  {} {} file {}: missing", "✗".red(), key, file_path);
                    issues += 1;
                }
                FileStatus::Broken => {
                    println!("  {} {} file {}: broken symlink", "✗".red(), key, file_path);
                    issues += 1;
                }
                FileStatus::Wrong(target) => {
                    println!(
                        "  {} {} file {}: points to {} (expected central)",
                        "✗".red(),
                        key,
                        file_path,
                        target
                    );
                    issues += 1;
                }
                FileStatus::Unmanaged => {
                    println!(
                        "  {} {} file {}: not yet linked (run `agm link`)",
                        "warn".yellow(),
                        key,
                        file_path
                    );
                }
                FileStatus::ReadyToLink => {
                    println!(
                        "  {} {} file {}: central exists, symlink missing (run `agm link`)",
                        "warn".yellow(),
                        key,
                        file_path
                    );
                }
            }
        }
    }

    // Check central-level files
    for file_path in &config.central.files {
        let original = expand_path(file_path);
        let label = contract_tilde(&original);
        match check_file_status(&original, &files_base) {
            FileStatus::Linked => {}
            FileStatus::Missing => {
                println!("  {} central file {}: missing", "✗".red(), label);
                issues += 1;
            }
            FileStatus::Broken => {
                println!("  {} central file {}: broken symlink", "✗".red(), label);
                issues += 1;
            }
            FileStatus::Wrong(target) => {
                println!(
                    "  {} central file {}: points to {} (expected central)",
                    "✗".red(),
                    label,
                    target
                );
                issues += 1;
            }
            FileStatus::Unmanaged => {
                println!(
                    "  {} central file {}: not yet linked (run `agm link`)",
                    "warn".yellow(),
                    label
                );
            }
            FileStatus::ReadyToLink => {
                println!(
                    "  {} central file {}: central exists, symlink missing (run `agm link`)",
                    "warn".yellow(),
                    label
                );
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
