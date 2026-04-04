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

        let prompt_link = tool.resolved_link_path("prompt");
        let prompt_ls = prompt_link
            .as_ref()
            .map(|l| check_link(l, &central_prompt, false));

        let skills_link = tool.resolved_link_path("skills");
        let skills_ls = skills_link
            .as_ref()
            .map(|l| check_link(l, &central_skills, true));

        let agents_link = tool.resolved_link_path("agents");
        let agents_ls = agents_link
            .as_ref()
            .map(|l| check_link(l, &central_agents, true));

        let commands_link = tool.resolved_link_path("commands");
        let commands_ls = commands_link
            .as_ref()
            .map(|l| check_link(l, &central_commands, true));

        let config_dir = contract_tilde(&tool.resolved_config_dir());
        println!(
            " {:<13} {:<23}",
            format!("{} ({})", key, tool.name).dimmed(),
            config_dir.dimmed(),
        );

        if let Some(ls) = prompt_ls {
            print!("{}{:<8}", INDENT, "prompt");
            if config.central.is_disabled("prompt") {
                println!("{}", "disabled".dimmed());
            } else {
                match ls {
                    LinkStatus::Linked => println!(
                        "{} → {}",
                        "✓ linked".green(),
                        contract_tilde(prompt_link.as_ref().unwrap()).dimmed()
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
                        contract_tilde(prompt_link.as_ref().unwrap()).dimmed()
                    ),
                }
            }
        }

        if let Some(ls) = skills_ls {
            print!("{}{:<8}", INDENT, "skills");
            if config.central.is_disabled("skills") {
                println!("{}", "disabled".dimmed());
            } else {
                match ls {
                    LinkStatus::Linked => println!(
                        "{} → {}",
                        "✓ linked".green(),
                        contract_tilde(skills_link.as_ref().unwrap()).dimmed()
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
                        contract_tilde(skills_link.as_ref().unwrap()).dimmed()
                    ),
                }
            }
        }

        if let Some(ls) = agents_ls {
            print!("{}{:<8}", INDENT, "agents");
            if config.central.is_disabled("agents") {
                println!("{}", "disabled".dimmed());
            } else {
                match ls {
                    LinkStatus::Linked => println!(
                        "{} → {}",
                        "✓ linked".green(),
                        contract_tilde(agents_link.as_ref().unwrap()).dimmed()
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
                        contract_tilde(agents_link.as_ref().unwrap()).dimmed()
                    ),
                }
            }
        }

        if let Some(ls) = commands_ls {
            print!("{}{:<8}", INDENT, "commands");
            if config.central.is_disabled("commands") {
                println!("{}", "disabled".dimmed());
            } else {
                match ls {
                    LinkStatus::Linked => println!(
                        "{} → {}",
                        "✓ linked".green(),
                        contract_tilde(commands_link.as_ref().unwrap()).dimmed()
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
                        contract_tilde(commands_link.as_ref().unwrap()).dimmed()
                    ),
                }
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

    if config.central.is_disabled("prompt") {
        println!(
            "Central prompt : {} {}",
            contract_tilde(&central_prompt),
            "(disabled)".dimmed()
        );
    } else {
        println!("Central prompt : {}", contract_tilde(&central_prompt));
    }
    if config.central.is_disabled("skills") {
        println!(
            "Central skills : {} ({} installed, {} sources) {}",
            contract_tilde(&central_skills),
            installed_skills,
            groups.len(),
            "(disabled)".dimmed()
        );
    } else {
        println!(
            "Central skills : {} ({} installed, {} sources)",
            contract_tilde(&central_skills),
            installed_skills,
            groups.len()
        );
    }
    if config.central.is_disabled("agents") {
        println!(
            "Central agents : {} ({} installed) {}",
            contract_tilde(&central_agents),
            installed_agents,
            "(disabled)".dimmed()
        );
    } else {
        println!(
            "Central agents : {} ({} installed)",
            contract_tilde(&central_agents),
            installed_agents,
        );
    }
    if config.central.is_disabled("commands") {
        println!(
            "Central commands: {} ({} installed) {}",
            contract_tilde(&central_commands),
            installed_commands,
            "(disabled)".dimmed()
        );
    } else {
        println!(
            "Central commands: {} ({} installed)",
            contract_tilde(&central_commands),
            installed_commands,
        );
    }
    let source_dir = expand_tilde(&config.central.source_dir);
    println!("Central source : {}", contract_tilde(&source_dir));
    println!();

    Ok(())
}
