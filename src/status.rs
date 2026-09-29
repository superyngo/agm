use colored::Colorize;

use crate::config::Config;
use crate::linker::{check_link, LinkStatus};
use crate::paths::{contract_tilde, expand_tilde};
use crate::skills;

/// Display table with tool name, config dir, prompt/skills/agents link status and paths
pub fn status(config_path: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    let config = Config::load_from(config_path)?;
    let agm_skills = expand_tilde(&config.agm.skills_source);
    let agm_agents = expand_tilde(&config.agm.agents_source);
    let agm_commands = expand_tilde(&config.agm.commands_source);
    let agm_prompt = expand_tilde(&config.agm.prompt_source);

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

        let config_dir = contract_tilde(&tool.resolved_config_dir());
        println!(
            " {:<13} {:<23}",
            format!("{} ({})", key, tool.name).dimmed(),
            config_dir.dimmed(),
        );

        for (feature, source, is_dir) in [
            ("prompt", &agm_prompt, false),
            ("skills", &agm_skills, true),
            ("agents", &agm_agents, true),
            ("commands", &agm_commands, true),
        ] {
            let Some(link) = tool.resolved_link_path(feature) else {
                continue;
            };
            print!("{}{:<9}", INDENT, feature);
            if config.agm.is_disabled(feature) {
                println!("{}", "disabled".dimmed());
                continue;
            }
            match check_link(&link, source, is_dir) {
                LinkStatus::Linked => println!(
                    "{} → {}",
                    "✓ linked".green(),
                    contract_tilde(&link).dimmed()
                ),
                LinkStatus::Missing => println!(
                    "{} → {}",
                    "✗ missing".yellow(),
                    contract_tilde(source).dimmed()
                ),
                LinkStatus::Broken => println!("{}", "✗ broken".red()),
                LinkStatus::Wrong(t) => println!("{} → {}", "✗ wrong".red(), t.dimmed()),
                LinkStatus::Blocked => println!(
                    "{} → {}",
                    "✗ not linked".yellow(),
                    contract_tilde(&link).dimmed()
                ),
            }
        }
    }

    println!("{}", "═".repeat(62));

    // Count skills and agents from all sources
    let groups = skills::scan_all_sources(
        &expand_tilde(&config.agm.source_dir),
        &agm_skills,
        &agm_agents,
        &agm_commands,
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

    if config.agm.is_disabled("prompt") {
        println!(
            "agm prompt : {} {}",
            contract_tilde(&agm_prompt),
            "(disabled)".dimmed()
        );
    } else {
        println!("agm prompt : {}", contract_tilde(&agm_prompt));
    }
    if config.agm.is_disabled("skills") {
        println!(
            "agm skills : {} ({} installed, {} sources) {}",
            contract_tilde(&agm_skills),
            installed_skills,
            groups.len(),
            "(disabled)".dimmed()
        );
    } else {
        println!(
            "agm skills : {} ({} installed, {} sources)",
            contract_tilde(&agm_skills),
            installed_skills,
            groups.len()
        );
    }
    if config.agm.is_disabled("agents") {
        println!(
            "agm agents : {} ({} installed) {}",
            contract_tilde(&agm_agents),
            installed_agents,
            "(disabled)".dimmed()
        );
    } else {
        println!(
            "agm agents : {} ({} installed)",
            contract_tilde(&agm_agents),
            installed_agents,
        );
    }
    if config.agm.is_disabled("commands") {
        println!(
            "agm commands: {} ({} installed) {}",
            contract_tilde(&agm_commands),
            installed_commands,
            "(disabled)".dimmed()
        );
    } else {
        println!(
            "agm commands: {} ({} installed)",
            contract_tilde(&agm_commands),
            installed_commands,
        );
    }
    let source_dir = expand_tilde(&config.agm.source_dir);
    println!("agm source : {}", contract_tilde(&source_dir));
    println!();

    Ok(())
}
