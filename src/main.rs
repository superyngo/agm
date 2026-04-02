mod config;
mod editor;
mod init;
mod linker;
mod paths;
mod platform;
mod skills;
mod status;
mod tui;

use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "agm", about = "AI Agent Manager", disable_version_flag = true)]
struct Cli {
    /// Print version information
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// Override config file path
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize agm config and central directories
    Init,
    /// Manage tools, links, and configuration
    Tool {
        /// Link all tools (non-interactive)
        #[arg(short = 'l', long)]
        link: bool,

        /// Unlink all tools (non-interactive)
        #[arg(short = 'u', long)]
        unlink: bool,

        /// Show status table (non-interactive)
        #[arg(short = 's', long)]
        status: bool,
    },
    /// Manage source repos, skills, and agents
    Source {
        /// Add a source (URL or local path)
        #[arg(short = 'a', long = "add")]
        add: Option<String>,
        /// Update all source repos (git pull)
        #[arg(short = 'u', long = "update")]
        update: bool,
        /// List all skills and agents grouped by source
        #[arg(short = 'l', long = "list")]
        list: bool,
        /// Install all skills without prompting (used with --add)
        #[arg(long = "all")]
        all: bool,
    },
}

/// If there is only 1 skill, return it directly. If multiple and `all` is true, return all.
/// Otherwise show a MultiSelect dialog and return the selected skills.
fn select_skills_to_install(
    skills: &[(String, PathBuf)],
    all: bool,
) -> anyhow::Result<Vec<(String, PathBuf)>> {
    if skills.len() <= 1 || all {
        return Ok(skills.to_vec());
    }

    use dialoguer::{theme::ColorfulTheme, MultiSelect};

    let labels: Vec<&str> = skills.iter().map(|(name, _)| name.as_str()).collect();
    let defaults: Vec<bool> = vec![true; skills.len()];

    let selected = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Select skills to install ({} found)", skills.len()))
        .items(&labels)
        .defaults(&defaults)
        .interact()?;

    Ok(selected.into_iter().map(|i| skills[i].clone()).collect())
}

fn prompt_yes_no(prompt: &str) -> bool {
    print!("{} [y/N]: ", prompt);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().to_lowercase();

    input == "y" || input == "yes"
}

fn link_all(config: &config::Config, _config_path: Option<&std::path::Path>) -> anyhow::Result<()> {
    let central_skills = paths::expand_tilde(&config.central.skills_source);
    let central_agents = paths::expand_tilde(&config.central.agents_source);
    let central_prompt = paths::expand_tilde(&config.central.prompt_source);
    let source_dir = paths::expand_tilde(&config.central.source_dir);
    let yes = true; // Non-interactive mode

    // Collect which tools to link (all installed tools)
    let tools_to_link: Vec<(&String, &config::ToolConfig)> = config
        .tools
        .iter()
        .filter(|(_, tc)| tc.is_installed())
        .collect();

    // Prune broken skill/agent links from central store
    if central_skills.is_dir() {
        let pruned = skills::prune_broken_skills(&central_skills)?;
        if pruned > 0 {
            println!(
                "{} Removed {} broken skill link(s)",
                "warn".yellow(),
                pruned
            );
        }
    }
    if central_agents.is_dir() {
        let pruned = skills::prune_broken_agents(&central_agents)?;
        if pruned > 0 {
            println!(
                "{} Removed {} broken agent link(s)",
                "warn".yellow(),
                pruned
            );
        }
    }

    // Process source_repos when linking
    if !config.central.source_repos.is_empty() {
        println!("\n{}", "Processing source repositories...".bold());
        for url in &config.central.source_repos {
            match skills::clone_or_pull(url, &source_dir) {
                Ok((_repo_path, found_skills)) => {
                    let mut count = 0;
                    for (name, skill_path) in &found_skills {
                        if let Ok(()) = skills::install_skill(name, skill_path, &central_skills) {
                            count += 1;
                        }
                    }
                    // Also install agents from the repo
                    let found_agents = skills::scan_agents(&_repo_path);
                    let mut agent_count = 0;
                    for (name, agent_path) in &found_agents {
                        if let Ok(()) = skills::install_agent(name, agent_path, &central_agents) {
                            agent_count += 1;
                        }
                    }
                    if count > 0 || agent_count > 0 {
                        println!(
                            "  {} {} skill(s), {} agent(s) from {}",
                            " ok ".green(),
                            count,
                            agent_count,
                            url
                        );
                    }
                }
                Err(e) => {
                    println!("  {} Failed to process {}: {}", "warn".red(), url, e);
                }
            }
        }
    }

    // Link tools
    for (key, tool) in tools_to_link {
        println!("\n{} ({}):", key, tool.name);

        // Link skills directory
        if !tool.skills_dir.is_empty() {
            let skills_link = tool.resolved_config_dir().join(&tool.skills_dir);

            if platform::is_dir_link(&skills_link) {
                let actual_target = fs::read_link(&skills_link)?;
                let expected_target = central_skills
                    .canonicalize()
                    .unwrap_or_else(|_| central_skills.clone());
                let resolved_actual = skills_link
                    .parent()
                    .map(|p: &std::path::Path| p.join(&actual_target))
                    .unwrap_or_else(|| actual_target.clone());
                let resolved_actual = resolved_actual.canonicalize().unwrap_or(resolved_actual);

                if resolved_actual != expected_target {
                    if yes
                        || prompt_yes_no(&format!(
                            "Skills already linked to {}. Re-link to AGM?",
                            paths::contract_tilde(&resolved_actual)
                        ))
                    {
                        platform::remove_link(&skills_link)?;
                        println!("  {} Removed old link", " ok ".green());
                    } else {
                        println!("  {} Skipping skills link", "skip".yellow());
                        continue;
                    }
                }
            } else if skills_link.is_dir() {
                let skills_content = skills::scan_skills(&skills_link);
                if !skills_content.is_empty() {
                    if yes
                        || prompt_yes_no(&format!(
                            "Found {} existing skill(s) in {}. Migrate to AGM and create link?",
                            skills_content.len(),
                            paths::contract_tilde(&skills_link)
                        ))
                    {
                        let tool_skills_target = source_dir.join("agm_tools").join(key);
                        let added = skills::migrate_tool_dir(
                            &skills_link,
                            &tool_skills_target,
                            &central_skills,
                            key,
                        )?;
                        if added > 0 {
                            println!("  {} Migrated {} skill(s)", " ok ".green(), added);
                        }
                    } else {
                        println!("  {} Skipping skills migration", "skip".yellow());
                        continue;
                    }
                } else {
                    // Empty skills dir — remove it so we can create the link
                    fs::remove_dir_all(&skills_link)?;
                }
            }

            linker::create_link(&skills_link, &central_skills, "skills", true)?;
        }

        // Link agents directory
        if !tool.agents_dir.is_empty() {
            let agents_link = tool.resolved_config_dir().join(&tool.agents_dir);

            if platform::is_dir_link(&agents_link) {
                let actual_target = fs::read_link(&agents_link)?;
                let expected_target = central_agents
                    .canonicalize()
                    .unwrap_or_else(|_| central_agents.clone());
                let resolved_actual = agents_link
                    .parent()
                    .map(|p: &std::path::Path| p.join(&actual_target))
                    .unwrap_or_else(|| actual_target.clone());
                let resolved_actual = resolved_actual.canonicalize().unwrap_or(resolved_actual);

                if resolved_actual != expected_target {
                    if yes
                        || prompt_yes_no(&format!(
                            "Agents already linked to {}. Re-link to AGM?",
                            paths::contract_tilde(&resolved_actual)
                        ))
                    {
                        platform::remove_link(&agents_link)?;
                        println!("  {} Removed old agents link", " ok ".green());
                    } else {
                        println!("  {} Skipping agents link", "skip".yellow());
                        continue;
                    }
                }
            } else if agents_link.is_dir() {
                // Existing agents dir — remove empty or warn
                let has_files = fs::read_dir(&agents_link)
                    .map(|rd| rd.count() > 0)
                    .unwrap_or(false);
                if has_files {
                    if yes
                        || prompt_yes_no(&format!(
                            "Existing agents dir at {}. Remove and create link?",
                            paths::contract_tilde(&agents_link)
                        ))
                    {
                        fs::remove_dir_all(&agents_link)?;
                    } else {
                        println!("  {} Skipping agents link", "skip".yellow());
                        continue;
                    }
                } else {
                    fs::remove_dir_all(&agents_link)?;
                }
            }

            linker::create_link(&agents_link, &central_agents, "agents", true)?;
        }

        // Link prompt file
        if !tool.prompt_filename.is_empty() {
            let prompt_link = tool.resolved_config_dir().join(&tool.prompt_filename);

            // Check if prompt is already correctly linked (symlink or hardlink)
            let already_linked = prompt_link.exists()
                && central_prompt.exists()
                && platform::same_file(&prompt_link, &central_prompt).unwrap_or(false);

            if !already_linked && prompt_link.exists() {
                if fs::read_link(&prompt_link).is_ok() {
                    // It's a symlink to wrong target
                    let actual_target = fs::read_link(&prompt_link)?;
                    let resolved_actual = prompt_link
                        .parent()
                        .map(|p: &std::path::Path| p.join(&actual_target))
                        .unwrap_or_else(|| actual_target.clone());
                    let resolved_actual = resolved_actual.canonicalize().unwrap_or(resolved_actual);

                    if yes
                        || prompt_yes_no(&format!(
                            "Prompt already linked to {}. Re-link to AGM?",
                            paths::contract_tilde(&resolved_actual)
                        ))
                    {
                        fs::remove_file(&prompt_link)?;
                        println!("  {} Removed old link", " ok ".green());
                    } else {
                        println!("  {} Skipping prompt link", "skip".yellow());
                        continue;
                    }
                } else {
                    // Regular file (not a link)
                    let content = fs::read_to_string(&prompt_link)?;
                    if !content.trim().is_empty() {
                        if yes
                            || prompt_yes_no(&format!(
                                "Existing prompt file found at {}. Backup and create link?",
                                paths::contract_tilde(&prompt_link)
                            ))
                        {
                            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                            let backup_path =
                                prompt_link.with_extension(format!("{}.bak", timestamp));
                            fs::rename(&prompt_link, &backup_path)?;
                            println!(
                                "  {} Backed up prompt to {}",
                                " ok ".green(),
                                paths::contract_tilde(&backup_path)
                            );
                        } else {
                            println!("  {} Skipping prompt link", "skip".yellow());
                            continue;
                        }
                    } else {
                        // Empty file — safe to remove without backup
                        fs::remove_file(&prompt_link)?;
                    }
                }
            }

            linker::create_link(&prompt_link, &central_prompt, "prompt", false)?;
        }
    }

    Ok(())
}

fn unlink_all(config: &config::Config) -> anyhow::Result<()> {
    let central_skills = paths::expand_tilde(&config.central.skills_source);
    let central_agents = paths::expand_tilde(&config.central.agents_source);
    let central_prompt = paths::expand_tilde(&config.central.prompt_source);

    // Collect which tools to unlink (all installed tools)
    let tools_to_unlink: Vec<(&String, &config::ToolConfig)> = config
        .tools
        .iter()
        .filter(|(_, tc)| tc.is_installed())
        .collect();

    for (key, tool_config) in tools_to_unlink {
        println!("Unlinking {} ({}):", key, tool_config.name);

        // Remove skills link then copy central skills back
        if !tool_config.skills_dir.is_empty() {
            let skills_link = tool_config
                .resolved_config_dir()
                .join(&tool_config.skills_dir);
            if linker::remove_link(&skills_link, "skills", true)? && central_skills.is_dir() {
                skills::copy_dir_all(&central_skills, &skills_link)?;
                println!("  {} skills copied back", " ok ".green());
            }
        }

        // Remove agents link then copy central agents back
        if !tool_config.agents_dir.is_empty() {
            let agents_link = tool_config
                .resolved_config_dir()
                .join(&tool_config.agents_dir);
            if linker::remove_link(&agents_link, "agents", true)? && central_agents.is_dir() {
                skills::copy_dir_all(&central_agents, &agents_link)?;
                println!("  {} agents copied back", " ok ".green());
            }
        }

        // Remove prompt link then copy central prompt back
        if !tool_config.prompt_filename.is_empty() {
            let prompt_link = tool_config
                .resolved_config_dir()
                .join(&tool_config.prompt_filename);
            if linker::remove_link(&prompt_link, "prompt", false)? && central_prompt.exists() {
                fs::copy(&central_prompt, &prompt_link)?;
                println!("  {} prompt copied back", " ok ".green());
            }
        }
    }

    Ok(())
}

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
    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            // No subcommand provided - show help and exit
            let mut cmd = Cli::command();
            cmd.print_help()?;
            println!();
            std::process::exit(1);
        }
    };

    match command {
        Commands::Init => init::run(cli.config.clone()),
        Commands::Tool {
            link,
            unlink,
            status,
        } => {
            // Enforce mutual exclusivity
            let flag_count = [link, unlink, status].iter().filter(|&&x| x).count();
            if flag_count > 1 {
                anyhow::bail!("Only one of --link, --unlink, --status can be specified");
            }

            if link {
                // Reuse existing link-all logic
                let config = config::Config::load_from(cli.config.clone())?;
                link_all(&config, cli.config.as_deref())?;
            } else if unlink {
                let config = config::Config::load_from(cli.config.clone())?;
                unlink_all(&config)?;
            } else if status {
                status::status()?;
            } else {
                // No flags → launch TUI
                tui::tool::run(cli.config.clone())?;
            }
            Ok(())
        }
        Commands::Source {
            add,
            update,
            list,
            all,
        } => {
            let mut config = config::Config::load_from(cli.config.clone())?;
            let skills_dir = paths::expand_tilde(&config.central.skills_source);
            let agents_dir = paths::expand_tilde(&config.central.agents_source);
            let commands_dir = paths::expand_tilde(&config.central.commands_source);
            let source_dir = paths::expand_tilde(&config.central.source_dir);

            if let Some(source) = add {
                // --add: add a source repo or local path
                if skills::is_url(&source) {
                    let (repo_path, found_skills) = skills::clone_or_pull(&source, &source_dir)?;
                    config.add_source_repo(&source)?;
                    let to_install = select_skills_to_install(&found_skills, all)?;
                    let mut count = 0;
                    for (name, skill_path) in &to_install {
                        match skills::install_skill(name, skill_path, &skills_dir) {
                            Ok(()) => {
                                println!(
                                    "  {} {} → {}",
                                    " ok ".green(),
                                    name,
                                    paths::contract_tilde(skill_path)
                                );
                                count += 1;
                            }
                            Err(e) => println!("  {} {}: {}", "warn".yellow(), name, e),
                        }
                    }
                    // Auto-install agents from the repo
                    let found_agents = skills::scan_agents(&repo_path);
                    let mut agent_count = 0;
                    for (name, agent_path) in &found_agents {
                        match skills::install_agent(name, agent_path, &agents_dir) {
                            Ok(()) => {
                                println!(
                                    "  {} agent {} → {}",
                                    " ok ".green(),
                                    name,
                                    paths::contract_tilde(agent_path)
                                );
                                agent_count += 1;
                            }
                            Err(e) => println!("  {} agent {}: {}", "warn".yellow(), name, e),
                        }
                    }
                    println!(
                        "\n{} skill(s), {} agent(s) installed from {}.",
                        count,
                        agent_count,
                        paths::contract_tilde(&repo_path)
                    );
                } else {
                    let source_path = paths::expand_tilde(&source);
                    println!(
                        "Adding skills from {}...",
                        paths::contract_tilde(&source_path)
                    );
                    let (dest, found_skills) = skills::add_local_copy(&source_path, &source_dir)?;
                    let to_install = select_skills_to_install(&found_skills, all)?;
                    let mut count = 0;
                    for (name, skill_path) in &to_install {
                        match skills::install_skill(name, skill_path, &skills_dir) {
                            Ok(()) => {
                                println!(
                                    "  {} {} → {}",
                                    " ok ".green(),
                                    name,
                                    paths::contract_tilde(skill_path)
                                );
                                count += 1;
                            }
                            Err(e) => println!("  {} {}: {}", "warn".yellow(), name, e),
                        }
                    }
                    println!(
                        "\n{} skill(s) installed from {}.",
                        count,
                        paths::contract_tilde(&dest)
                    );
                }
                Ok(())
            } else if update {
                // --update: git pull all repos
                skills::update_all(&skills_dir, &agents_dir, &source_dir)?;
                Ok(())
            } else if list {
                // --list: show all skills and agents
                let pruned = skills::prune_broken_skills(&skills_dir)?;
                if pruned > 0 {
                    println!(
                        "  {} Removed {} broken skill link(s)",
                        "warn".yellow(),
                        pruned
                    );
                }
                let pruned_agents = skills::prune_broken_agents(&agents_dir)?;
                if pruned_agents > 0 {
                    println!(
                        "  {} Removed {} broken agent link(s)",
                        "warn".yellow(),
                        pruned_agents
                    );
                }
                let pruned_commands = skills::prune_broken_commands(&commands_dir)?;
                if pruned_commands > 0 {
                    println!(
                        "  {} Removed {} broken command link(s)",
                        "warn".yellow(),
                        pruned_commands
                    );
                }
                let groups = skills::scan_all_sources(
                    &source_dir,
                    &skills_dir,
                    &agents_dir,
                    &commands_dir,
                    &config.central.source_repos,
                );
                if groups.is_empty() {
                    println!("No sources found. Use 'agm source --add <url>' to add a source.");
                } else {
                    println!();
                    let mut total_skills = 0;
                    let mut installed_skills = 0;
                    let mut total_agents = 0;
                    let mut installed_agents = 0;
                    for group in &groups {
                        let icon = match &group.kind {
                            skills::SourceKind::Repo { .. } => "📦",
                            skills::SourceKind::Local => "📁",
                            skills::SourceKind::Migrated { .. } => "📁",
                        };
                        let detail = match &group.kind {
                            skills::SourceKind::Repo { url } => url
                                .as_deref()
                                .map(|u| format!("repo: {}", u))
                                .unwrap_or_else(|| "repo".into()),
                            skills::SourceKind::Local => "local".into(),
                            skills::SourceKind::Migrated { tool } => {
                                format!("migrated from {}", tool)
                            }
                        };
                        println!("{} {} ({})", icon, group.name.bold(), detail);

                        if !group.skills.is_empty() {
                            println!("  {}", "Skills:".dimmed());
                            for skill in &group.skills {
                                total_skills += 1;
                                let (indicator, status_text) = match skill.install_status {
                                    skills::SkillInstallStatus::Installed => {
                                        installed_skills += 1;
                                        ("✓".green().to_string(), "installed".green().to_string())
                                    }
                                    skills::SkillInstallStatus::NotInstalled => (
                                        "✗".dimmed().to_string(),
                                        "not installed".dimmed().to_string(),
                                    ),
                                    skills::SkillInstallStatus::Conflict => {
                                        ("⚡".yellow().to_string(), "conflict".yellow().to_string())
                                    }
                                };
                                println!("   {} {:<24} {}", indicator, skill.name, status_text);
                            }
                        }

                        if !group.agents.is_empty() {
                            println!("  {}", "Agents:".dimmed());
                            for agent in &group.agents {
                                total_agents += 1;
                                let (indicator, status_text) = match agent.install_status {
                                    skills::SkillInstallStatus::Installed => {
                                        installed_agents += 1;
                                        ("✓".green().to_string(), "installed".green().to_string())
                                    }
                                    skills::SkillInstallStatus::NotInstalled => (
                                        "✗".dimmed().to_string(),
                                        "not installed".dimmed().to_string(),
                                    ),
                                    skills::SkillInstallStatus::Conflict => {
                                        ("⚡".yellow().to_string(), "conflict".yellow().to_string())
                                    }
                                };
                                println!("   {} {:<24} {}", indicator, agent.name, status_text);
                            }
                        }
                        println!();
                    }
                    println!(
                        "── {} ──",
                        format!(
                            "{} source(s), {} skill(s) ({} installed), {} agent(s) ({} installed)",
                            groups.len(),
                            total_skills,
                            installed_skills,
                            total_agents,
                            installed_agents,
                        )
                        .bold()
                    );
                }
                Ok(())
            } else {
                // No flags — enter TUI
                tui::source::run(&mut config)?;
                Ok(())
            }
        }
    }
}
