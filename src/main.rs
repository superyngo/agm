use agm::{config, editor, init, linker, paths, platform, skills, status, tui};

use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;
use std::fs;
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
    /// Initialize agm config and agm directories
    Init,
    /// Open the agm config file in $EDITOR (or the platform default)
    Config,
    /// Manage tools, links, and configuration
    Tool {
        #[command(subcommand)]
        action: Option<ToolAction>,
    },
    /// Manage source repos, skills, and agents
    Source {
        #[command(subcommand)]
        action: Option<SourceAction>,
    },
}

#[derive(Subcommand)]
enum ToolAction {
    /// Link all installed tools (non-interactive)
    Link,
    /// Unlink all installed tools (non-interactive)
    Unlink,
    /// Show status table (non-interactive)
    Status,
}

#[derive(Subcommand)]
enum SourceAction {
    /// Add a source (URL or local path)
    Add {
        source: String,
        /// Override target directory name
        #[arg(short = 'n', long)]
        name: Option<String>,
        /// Install all skills without prompting
        #[arg(long)]
        all: bool,
    },
    /// Update all source repos (git pull)
    Update,
    /// List all skills/agents grouped by source
    List,
    /// Delete a source by folder name or repo URL
    Del { target: String },
    /// Rename a source folder
    Rename { old: String, new: String },
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

fn print_clone_progress(evt: &skills::CloneProgress) {
    use skills::CloneProgress::*;
    match evt {
        Start { name, url, action } => {
            let verb = match action {
                skills::CloneAction::Clone => "Cloning",
                skills::CloneAction::Pull => "Updating",
            };
            println!("{} {} from {}...", verb, name, url);
        }
        GitLine { line, is_err } => {
            if *is_err {
                eprintln!("{}", line);
            } else {
                println!("{}", line);
            }
        }
        Done {
            success, message, ..
        } => {
            if *success {
                println!("{} {}", " ok ".green(), message);
            } else {
                println!("{} {}", "fail".red(), message);
            }
        }
    }
}

/// A linkable part of a tool's config: the prompt file or a directory of items.
struct Feature {
    key: &'static str,
    is_dir: bool,
}

const FEATURES: [Feature; 4] = [
    Feature {
        key: "prompt",
        is_dir: false,
    },
    Feature {
        key: "skills",
        is_dir: true,
    },
    Feature {
        key: "agents",
        is_dir: true,
    },
    Feature {
        key: "commands",
        is_dir: true,
    },
];

fn feature_source(config: &config::Config, feature: &Feature) -> std::path::PathBuf {
    let raw = match feature.key {
        "prompt" => &config.agm.prompt_source,
        "skills" => &config.agm.skills_source,
        "agents" => &config.agm.agents_source,
        _ => &config.agm.commands_source,
    };
    paths::expand_tilde(raw)
}

/// Resolve where an existing link points, relative links resolved against its parent.
fn resolved_link_target(link: &std::path::Path) -> Option<std::path::PathBuf> {
    let target = fs::read_link(link).ok()?;
    Some(linker::resolve_link_target(link, target))
}

/// Clear whatever is at `link` so a fresh link can be created, without destroying content:
/// foreign links are replaced, real directories are migrated into the store (leftovers
/// are kept as `.bak`), real prompt files are backed up.
fn prepare_link(
    config: &config::Config,
    key: &str,
    tool: &config::ToolConfig,
    feature: &Feature,
    link: &std::path::Path,
    source: &std::path::Path,
) -> anyhow::Result<()> {
    use anyhow::Context;

    let is_link = platform::is_dir_link(link) || fs::read_link(link).is_ok();
    if is_link {
        let expected = source
            .canonicalize()
            .unwrap_or_else(|_| source.to_path_buf());
        match resolved_link_target(link) {
            Some(actual) if actual == expected => {}
            other => {
                if feature.is_dir {
                    platform::remove_link(link)
                } else {
                    fs::remove_file(link)
                }
                .with_context(|| format!("removing old {} link {}", feature.key, link.display()))?;
                println!(
                    "  {} Removed old {} link{}",
                    " ok ".green(),
                    feature.key,
                    other
                        .map(|t| format!(" (pointed to {})", paths::contract_tilde(&t)))
                        .unwrap_or_default()
                );
            }
        }
        return Ok(());
    }
    if link.symlink_metadata().is_err() {
        return Ok(());
    }

    if !feature.is_dir {
        if platform::same_file(link, source).unwrap_or(false) {
            return Ok(());
        }
        let content = fs::read_to_string(link).unwrap_or_default();
        if content.trim().is_empty() {
            fs::remove_file(link)?;
        } else {
            let backup = linker::backup_path(link);
            fs::rename(link, &backup)?;
            println!(
                "  {} Backed up prompt to {}",
                " ok ".green(),
                paths::contract_tilde(&backup)
            );
        }
        return Ok(());
    }

    let source_dir = paths::expand_tilde(&config.agm.source_dir);
    let (added, msgs) = skills::migrate_feature_dir(
        feature.key,
        link,
        &source_dir,
        source,
        key,
        &tool.prompt_filename,
    )?;
    for m in &msgs {
        println!("{}", m);
    }
    if added > 0 {
        println!("  {} Migrated {} {}", " ok ".green(), added, feature.key);
    }
    Ok(())
}

fn link_tool(config: &config::Config, key: &str, tool: &config::ToolConfig) -> anyhow::Result<()> {
    for feature in &FEATURES {
        if config.agm.is_disabled(feature.key) {
            continue;
        }
        let Some(link) = tool.resolved_link_path(feature.key) else {
            continue;
        };
        let source = feature_source(config, feature);
        prepare_link(config, key, tool, feature, &link, &source)?;
        linker::create_link(&link, &source, feature.key, feature.is_dir)?;
    }
    Ok(())
}

/// Run `f` for every installed tool, keep going after a failure, and fail at the end
/// with a summary so one broken tool never leaves the others untouched.
fn for_each_installed_tool(
    config: &config::Config,
    heading: &str,
    f: impl Fn(&str, &config::ToolConfig) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut failures = Vec::new();
    for (key, tool) in config.tools.iter().filter(|(_, tc)| tc.is_installed()) {
        println!("\n{}{} ({}):", heading, key, tool.name);
        if let Err(e) = f(key, tool) {
            println!("  {} {:#}", "fail".red(), e);
            failures.push(format!("{}: {:#}", key, e));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(
            "{} tool(s) failed:\n  {}",
            failures.len(),
            failures.join("\n  ")
        )
    }
}

fn link_all(config: &config::Config) -> anyhow::Result<()> {
    let agm_skills = paths::expand_tilde(&config.agm.skills_source);
    let agm_agents = paths::expand_tilde(&config.agm.agents_source);
    let agm_commands = paths::expand_tilde(&config.agm.commands_source);

    // Prune broken links from the agm stores
    for (what, dir, prune) in [
        (
            "skill",
            &agm_skills,
            skills::prune_broken_skills as fn(&std::path::Path) -> anyhow::Result<usize>,
        ),
        ("agent", &agm_agents, skills::prune_broken_agents),
        ("command", &agm_commands, skills::prune_broken_commands),
    ] {
        if dir.is_dir() {
            let pruned = prune(dir)?;
            if pruned > 0 {
                println!(
                    "{} Removed {} broken {} link(s)",
                    "warn".yellow(),
                    pruned,
                    what
                );
            }
        }
    }

    if !config.agm.disabled.is_empty() {
        println!(
            "\n{} Disabled features: {}",
            "note".yellow(),
            config.agm.disabled.join(", ")
        );
    }
    for_each_installed_tool(config, "", |key, tool| link_tool(config, key, tool))
}

fn unlink_tool(config: &config::Config, tool: &config::ToolConfig) -> anyhow::Result<()> {
    for feature in &FEATURES {
        if config.agm.is_disabled(feature.key) {
            continue;
        }
        let Some(link) = tool.resolved_link_path(feature.key) else {
            continue;
        };
        let source = feature_source(config, feature);
        if linker::remove_link(&link, feature.key, feature.is_dir)?
            && linker::detach_copy(&source, &link, feature.is_dir)?
        {
            println!("  {} {} copied back", " ok ".green(), feature.key);
        }
    }
    Ok(())
}

fn unlink_all(config: &config::Config) -> anyhow::Result<()> {
    for_each_installed_tool(config, "Unlinking ", |_, tool| unlink_tool(config, tool))
}

/// Install every agent and command found under `root`; returns (agents, commands) installed.
fn install_agents_and_commands(
    root: &std::path::Path,
    agents_dir: &std::path::Path,
    commands_dir: &std::path::Path,
) -> (usize, usize) {
    let mut agents = 0;
    for (n, p) in &skills::scan_agents(root) {
        match skills::install_agent(n, p, agents_dir) {
            Ok(()) => {
                println!(
                    "  {} agent {} → {}",
                    " ok ".green(),
                    n,
                    paths::contract_tilde(p)
                );
                agents += 1;
            }
            Err(e) => println!("  {} agent {}: {}", "warn".yellow(), n, e),
        }
    }
    let mut commands = 0;
    for (n, p) in &skills::scan_commands(root) {
        match skills::install_command(n, p, commands_dir) {
            Ok(()) => {
                println!(
                    "  {} command {} → {}",
                    " ok ".green(),
                    n,
                    paths::contract_tilde(p)
                );
                commands += 1;
            }
            Err(e) => println!("  {} command {}: {}", "warn".yellow(), n, e),
        }
    }
    (agents, commands)
}

fn source_add(
    source: &str,
    name: Option<&str>,
    all: bool,
    source_dir: &std::path::Path,
    skills_dir: &std::path::Path,
    agents_dir: &std::path::Path,
    commands_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let normalized = skills::normalize_git_source(source);
    if skills::is_url(&normalized) {
        let (repo_path, found_skills) =
            skills::clone_or_pull(&normalized, source_dir, name, |evt| {
                print_clone_progress(&evt)
            })?;
        let to_install = select_skills_to_install(&found_skills, all)?;
        let mut count = 0;
        for (n, p) in &to_install {
            match skills::install_skill(n, p, skills_dir) {
                Ok(()) => {
                    println!("  {} {} → {}", " ok ".green(), n, paths::contract_tilde(p));
                    count += 1;
                }
                Err(e) => println!("  {} {}: {}", "warn".yellow(), n, e),
            }
        }
        let (agent_count, command_count) =
            install_agents_and_commands(&repo_path, agents_dir, commands_dir);
        println!(
            "\n{} skill(s), {} agent(s), {} command(s) installed from {}.",
            count,
            agent_count,
            command_count,
            paths::contract_tilde(&repo_path)
        );
    } else {
        let source_path = paths::expand_tilde(source);
        println!(
            "Adding skills from {}...",
            paths::contract_tilde(&source_path)
        );
        let (dest, found_skills) = skills::add_local_copy(&source_path, source_dir, name, |evt| {
            print_clone_progress(&evt)
        })?;
        let to_install = select_skills_to_install(&found_skills, all)?;
        let mut count = 0;
        for (n, p) in &to_install {
            match skills::install_skill(n, p, skills_dir) {
                Ok(()) => {
                    println!("  {} {} → {}", " ok ".green(), n, paths::contract_tilde(p));
                    count += 1;
                }
                Err(e) => println!("  {} {}: {}", "warn".yellow(), n, e),
            }
        }
        let (agent_count, command_count) =
            install_agents_and_commands(&dest, agents_dir, commands_dir);
        println!(
            "\n{} skill(s), {} agent(s), {} command(s) installed from {}.",
            count,
            agent_count,
            command_count,
            paths::contract_tilde(&dest)
        );
    }
    Ok(())
}

fn source_update(
    skills_dir: &std::path::Path,
    agents_dir: &std::path::Path,
    commands_dir: &std::path::Path,
    source_dir: &std::path::Path,
) -> anyhow::Result<()> {
    use skills::UpdateProgress::*;
    skills::update_all_with_progress(
        skills_dir,
        agents_dir,
        commands_dir,
        source_dir,
        |p| match p {
            RepoStart { name } => println!("Updating {}...", name),
            RepoComplete {
                name,
                success,
                message,
            } => {
                let tag = if success {
                    " ok ".green()
                } else {
                    "fail".red()
                };
                println!("  {} {}: {}", tag, name, message);
            }
            AllDone {
                total,
                updated,
                new_skills,
                new_agents,
                new_commands,
            } => {
                println!(
                    "\nUpdated {}/{}; {} new skill(s), {} new agent(s), {} new command(s).",
                    updated, total, new_skills, new_agents, new_commands
                );
            }
        },
    );
    Ok(())
}

fn source_list(
    skills_dir: &std::path::Path,
    agents_dir: &std::path::Path,
    commands_dir: &std::path::Path,
    source_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let pruned = skills::prune_broken_skills(skills_dir)?;
    if pruned > 0 {
        println!(
            "  {} Removed {} broken skill link(s)",
            "warn".yellow(),
            pruned
        );
    }
    let pruned_agents = skills::prune_broken_agents(agents_dir)?;
    if pruned_agents > 0 {
        println!(
            "  {} Removed {} broken agent link(s)",
            "warn".yellow(),
            pruned_agents
        );
    }
    let pruned_commands = skills::prune_broken_commands(commands_dir)?;
    if pruned_commands > 0 {
        println!(
            "  {} Removed {} broken command link(s)",
            "warn".yellow(),
            pruned_commands
        );
    }
    let groups = skills::scan_all_sources(source_dir, skills_dir, agents_dir, commands_dir);
    if groups.is_empty() {
        println!("No sources found. Use 'agm source add <url>' to add a source.");
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
}

fn source_del(
    target: &str,
    source_dir: &std::path::Path,
    skills_dir: &std::path::Path,
    agents_dir: &std::path::Path,
    commands_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let group =
        skills::resolve_source_target(target, source_dir, skills_dir, agents_dir, commands_dir)?;
    skills::delete_source(&group, skills_dir, agents_dir, commands_dir)?;
    println!("{} Deleted source {}", " ok ".green(), group.name);
    Ok(())
}

fn source_rename(
    old: &str,
    new: &str,
    source_dir: &std::path::Path,
    skills_dir: &std::path::Path,
    agents_dir: &std::path::Path,
    commands_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let report = skills::rename_source(
        old,
        new,
        source_dir,
        skills_dir,
        agents_dir,
        commands_dir,
        |evt| print_clone_progress(&evt),
    )?;
    println!(
        "Relinked: {} skill(s), {} agent(s), {} command(s)",
        report.skills_relinked, report.agents_relinked, report.commands_relinked
    );
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
        None => return tui::shell::run(cli.config.clone(), tui::shell::Tab::Tool),
    };

    match command {
        Commands::Init => init::run(cli.config.clone()),
        Commands::Config => {
            let config = config::Config::load_from(cli.config.clone())?;
            let path = cli
                .config
                .clone()
                .unwrap_or_else(config::Config::config_path);
            let ed = editor::get_editor(&config);
            editor::open_files(&ed, &[&path])
        }
        Commands::Tool { action } => match action {
            None => tui::shell::run(cli.config.clone(), tui::shell::Tab::Tool),
            Some(ToolAction::Link) => {
                let config = config::Config::load_from(cli.config.clone())?;
                link_all(&config)
            }
            Some(ToolAction::Unlink) => {
                let config = config::Config::load_from(cli.config.clone())?;
                unlink_all(&config)
            }
            Some(ToolAction::Status) => status::status(cli.config.clone()),
        },
        Commands::Source { action } => {
            let config = config::Config::load_from(cli.config.clone())?;
            let skills_dir = paths::expand_tilde(&config.agm.skills_source);
            let agents_dir = paths::expand_tilde(&config.agm.agents_source);
            let commands_dir = paths::expand_tilde(&config.agm.commands_source);
            let source_dir = paths::expand_tilde(&config.agm.source_dir);
            match action {
                None => tui::shell::run(cli.config.clone(), tui::shell::Tab::Source),
                Some(SourceAction::Add { source, name, all }) => source_add(
                    &source,
                    name.as_deref(),
                    all,
                    &source_dir,
                    &skills_dir,
                    &agents_dir,
                    &commands_dir,
                ),
                Some(SourceAction::Update) => {
                    source_update(&skills_dir, &agents_dir, &commands_dir, &source_dir)
                }
                Some(SourceAction::List) => {
                    source_list(&skills_dir, &agents_dir, &commands_dir, &source_dir)
                }
                Some(SourceAction::Del { target }) => source_del(
                    &target,
                    &source_dir,
                    &skills_dir,
                    &agents_dir,
                    &commands_dir,
                ),
                Some(SourceAction::Rename { old, new }) => source_rename(
                    &old,
                    &new,
                    &source_dir,
                    &skills_dir,
                    &agents_dir,
                    &commands_dir,
                ),
            }
        }
    }
}
