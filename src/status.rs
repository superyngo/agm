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
            Some(check_link(&link, &central_prompt, false))
        } else {
            None
        };

        let skills_ls = if !tool.skills_dir.is_empty() {
            let link = tool.resolved_config_dir().join(&tool.skills_dir);
            Some(check_link(&link, &central_skills, true))
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
