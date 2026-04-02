use colored::Colorize;

use crate::config::Config;
use crate::linker::{check_link, LinkStatus};
use crate::paths::{contract_tilde, expand_tilde};
use crate::skills;

/// Display table with tool name, config dir, prompt/skills/agents link status and paths
pub fn status() -> anyhow::Result<()> {
    let config = Config::load()?;
    let central_skills = expand_tilde(&config.central.skills_source);
    let central_agents = expand_tilde(&config.central.agents_source);
    let central_commands = expand_tilde(&config.central.commands_source);
    let central_prompt = expand_tilde(&config.central.prompt_source);

    // Indent for detail lines: aligns under the data columns
    const INDENT: &str = "                ";

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

        let agents_ls = if !tool.agents_dir.is_empty() {
            let link = tool.resolved_config_dir().join(&tool.agents_dir);
            Some(check_link(&link, &central_agents, true))
        } else {
            None
        };

        let commands_ls = if !tool.commands_dir.is_empty() {
            let link = tool.resolved_config_dir().join(&tool.commands_dir);
            Some(check_link(&link, &central_commands, true))
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
                LinkStatus::Blocked => println!(
                    "{} → {}",
                    "✗ not linked".yellow(),
                    contract_tilde(&prompt_link).dimmed()
                ),
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
                LinkStatus::Blocked => println!(
                    "{} → {}",
                    "✗ not linked".yellow(),
                    contract_tilde(&skills_link).dimmed()
                ),
            }
        }

        // Detail lines: agents
        if let Some(ls) = agents_ls {
            let agents_link = tool.resolved_config_dir().join(&tool.agents_dir);
            print!("{}{:<8}", INDENT, "agents");
            match ls {
                LinkStatus::Linked => println!(
                    "{} → {}",
                    "✓ linked".green(),
                    contract_tilde(&agents_link).dimmed()
                ),
                LinkStatus::Missing => println!(
                    "{} → {}",
                    "✗ missing".yellow(),
                    contract_tilde(&central_agents).dimmed()
                ),
                LinkStatus::Broken => println!("{}", "✗ broken".red()),
                LinkStatus::Wrong(t) => println!("{} → {}", "✗ wrong".red(), t.dimmed()),
                LinkStatus::Blocked => println!(
                    "{} → {}",
                    "✗ not linked".yellow(),
                    contract_tilde(&agents_link).dimmed()
                ),
            }
        }

        // Detail lines: commands
        if let Some(ls) = commands_ls {
            let commands_link = tool.resolved_config_dir().join(&tool.commands_dir);
            print!("{}{:<8}", INDENT, "commands");
            match ls {
                LinkStatus::Linked => println!(
                    "{} → {}",
                    "✓ linked".green(),
                    contract_tilde(&commands_link).dimmed()
                ),
                LinkStatus::Missing => println!(
                    "{} → {}",
                    "✗ missing".yellow(),
                    contract_tilde(&central_commands).dimmed()
                ),
                LinkStatus::Broken => println!("{}", "✗ broken".red()),
                LinkStatus::Wrong(t) => println!("{} → {}", "✗ wrong".red(), t.dimmed()),
                LinkStatus::Blocked => println!(
                    "{} → {}",
                    "✗ not linked".yellow(),
                    contract_tilde(&commands_link).dimmed()
                ),
            }
        }
    }

    println!("{}", "═".repeat(62));

    // Count skills and agents from all sources
    let groups = skills::scan_all_sources(
        &expand_tilde(&config.central.source_dir),
        &central_skills,
        &central_agents,
        &central_commands,
        &config.central.source_repos,
    );
    let installed_skills: usize = groups
        .iter()
        .flat_map(|g| &g.skills)
        .filter(|s| s.install_status == skills::SkillInstallStatus::Installed)
        .count();
    let installed_agents: usize = groups
        .iter()
        .flat_map(|g| &g.agents)
        .filter(|a| a.install_status == skills::SkillInstallStatus::Installed)
        .count();
    let installed_commands: usize = groups
        .iter()
        .flat_map(|g| &g.commands)
        .filter(|c| c.install_status == skills::SkillInstallStatus::Installed)
        .count();

    println!("Central prompt : {}", contract_tilde(&central_prompt));
    println!(
        "Central skills : {} ({} installed, {} sources)",
        contract_tilde(&central_skills),
        installed_skills,
        groups.len()
    );
    println!(
        "Central agents : {} ({} installed)",
        contract_tilde(&central_agents),
        installed_agents,
    );
    println!(
        "Central commands: {} ({} installed)",
        contract_tilde(&central_commands),
        installed_commands,
    );
    let source_dir = expand_tilde(&config.central.source_dir);
    println!("Central source : {}", contract_tilde(&source_dir));
    println!();

    Ok(())
}
