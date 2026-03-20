mod config;
mod editor;
mod files;
mod init;
mod linker;
mod paths;
mod platform;
mod skills;
mod status;

use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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
    /// Create/repair links
    Link {
        /// Target: a tool name, "all" (all installed tools), or "central" (central files only)
        target: Option<String>,
        /// Skip all confirmation prompts
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
    /// Remove links for a tool
    Unlink {
        /// Target: a tool name, "all" (all installed tools), or "central" (central files only)
        target: Option<String>,
    },
    /// Edit config file (central = agm config.toml, or a tool key)
    Config { target: Option<String> },
    /// Edit prompt file (central = central MASTER.md, or a tool key)
    Prompt { target: Option<String> },
    /// Edit auth files for a tool
    Auth { target: Option<String> },
    /// Edit MCP config files for a tool
    Mcp { target: Option<String> },
    /// Manage skills
    Skills {
        #[command(subcommand)]
        action: Option<SkillsAction>,
    },
    /// Show link status for all tools
    Status,
}

#[derive(Subcommand)]
enum SkillsAction {
    /// List all installed skills
    List,
    /// Install skill(s) from local path or repo URL
    Add { source: String },
    /// Remove a skill
    Remove { name: String },
    /// Git pull all skill source repos
    Update,
}

fn pick_target(
    config: &config::Config,
    cmd: &str,
    include_central: bool,
) -> anyhow::Result<String> {
    use dialoguer::{theme::ColorfulTheme, Select};

    let mut keys: Vec<String> = Vec::new();
    let mut labels: Vec<String> = Vec::new();

    if include_central {
        keys.push("central".into());
        let desc = if cmd == "prompt" {
            "central MASTER.md"
        } else {
            "agm config.toml"
        };
        labels.push(format!("{:<14} {}", "central", desc));
    }
    for (key, tool) in &config.tools {
        keys.push(key.clone());
        labels.push(format!("{:<14} {}", key, tool.name));
    }

    if keys.is_empty() {
        anyhow::bail!("No targets available for `agm {}`", cmd);
    }

    let idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("agm {} — select target", cmd))
        .items(&labels)
        .default(0)
        .interact()?;

    Ok(keys[idx].clone())
}

fn pick_link_target(config: &config::Config, cmd: &str) -> anyhow::Result<String> {
    use dialoguer::{theme::ColorfulTheme, Select};

    let mut keys: Vec<String> = vec!["all".into(), "central".into()];
    let mut labels: Vec<String> = vec![
        format!("{:<14} {}", "all", "all installed tools"),
        format!("{:<14} {}", "central", "central files only"),
    ];
    for (key, tool) in &config.tools {
        keys.push(key.clone());
        labels.push(format!("{:<14} {}", key, tool.name));
    }

    let idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("agm {} — select target", cmd))
        .items(&labels)
        .default(0)
        .interact()?;

    Ok(keys[idx].clone())
}

fn pick_file(files: &[PathBuf]) -> anyhow::Result<&PathBuf> {
    if files.len() == 1 {
        return Ok(&files[0]);
    }
    use dialoguer::{theme::ColorfulTheme, Select};
    let items: Vec<String> = files.iter().map(|p| paths::contract_tilde(p)).collect();
    let idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select file to open")
        .items(&items)
        .default(0)
        .interact()?;
    Ok(&files[idx])
}

fn prompt_yes_no(prompt: &str) -> bool {
    print!("{} [y/N]: ", prompt);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().to_lowercase();

    input == "y" || input == "yes"
}

/// Migrate an existing skills directory into AGM's central store.
///
/// For each skill found in `skills_link`:
///   - If its name is already taken in `central_skills`, prefix it with `{tool_key}_`
///   - Move the skill dir to `$source_dir/agm_tools/{tool_key}/{effective_name}`
///   - Symlink it into `central_skills/{effective_name}`
///
/// Non-skill files/dirs left over are cleaned up with remove_dir_all.
/// Returns the number of skills migrated.
fn migrate_skills_dir(
    skills_link: &Path,
    tool_skills_target: &Path,
    central_skills: &Path,
    tool_key: &str,
) -> anyhow::Result<usize> {
    use anyhow::Context;

    fs::create_dir_all(tool_skills_target)?;
    fs::create_dir_all(central_skills)?;

    let discovered = skills::scan_skills(skills_link);
    let mut migrated = 0;

    for (name, skill_path) in &discovered {
        // Determine effective name — avoid collision in central, try {tool_key}_{name}
        let effective_name = if !central_skills.join(name).exists() {
            name.clone()
        } else {
            let prefixed = format!("{}_{}", tool_key, name);
            println!(
                "  {} skill '{}' already in central, renaming to '{}'",
                "warn".yellow(),
                name,
                prefixed
            );
            prefixed
        };

        let dest = tool_skills_target.join(&effective_name);
        let link = central_skills.join(&effective_name);

        // If dest already exists (previous partial run), skip the move
        if dest.exists() {
            println!(
                "  {} {} already in store, re-linking",
                "skip".yellow(),
                effective_name
            );
        } else {
            fs::rename(skill_path, &dest)
                .with_context(|| format!("Failed to move skill '{}' to store", effective_name))?;
        }

        // If central link already exists and points to dest, skip
        if link.symlink_metadata().is_ok() {
            let already_ok = platform::read_dir_link_target(&link)
                .and_then(|t| fs::canonicalize(&t).ok())
                .zip(fs::canonicalize(&dest).ok())
                .map(|(a, b)| a == b)
                .unwrap_or(false);
            if already_ok {
                println!("  {} {} already linked", "skip".yellow(), effective_name);
                migrated += 1;
                continue;
            }
            // Stale/wrong link — remove and recreate
            platform::remove_link(&link)?;
        }

        platform::link_dir(&dest, &link)
            .with_context(|| format!("Failed to link skill '{}' into central", effective_name))?;

        println!(
            "  {} {} → {}",
            " ok ".green(),
            effective_name,
            paths::contract_tilde(&dest)
        );
        migrated += 1;
    }

    // Clean up leftover files/dirs (non-skills, e.g. .DS_Store, README.md)
    if skills_link.exists() {
        fs::remove_dir_all(skills_link)?;
    }

    Ok(migrated)
}

fn copy_dir_all(src: &Path, dst: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let meta = fs::symlink_metadata(&src_path)?;
        if platform::is_dir_link(&src_path) {
            // Recreate directory link
            if let Some(target) = platform::read_dir_link_target(&src_path) {
                if dst_path.symlink_metadata().is_ok() {
                    platform::remove_link(&dst_path)?;
                }
                platform::link_dir(&target, &dst_path)?;
            }
        } else if meta.file_type().is_symlink() {
            // File symlink (Unix) — recreate
            if let Ok(target) = fs::read_link(&src_path) {
                if dst_path.symlink_metadata().is_ok() {
                    fs::remove_file(&dst_path)?;
                }
                platform::link_file(&target, &dst_path)?;
            }
        } else if meta.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn open_tool_files(
    config: &config::Config,
    ed: &str,
    tool_name: &str,
    file_type: &str,
) -> anyhow::Result<()> {
    let tool_config = config
        .tools
        .get(tool_name)
        .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found in config", tool_name))?;

    let base_dir = tool_config.resolved_config_dir();

    let files_to_open: Vec<PathBuf> = match file_type {
        "prompt" => {
            if tool_config.prompt_filename.is_empty() {
                anyhow::bail!("Tool '{}' has no prompt file configured", tool_name);
            }
            vec![base_dir.join(&tool_config.prompt_filename)]
        }
        "config" => tool_config
            .settings
            .iter()
            .map(|f| tool_config.resolve_path(f))
            .collect(),
        "auth" => tool_config
            .auth
            .iter()
            .map(|f| tool_config.resolve_path(f))
            .collect(),
        "mcp" => tool_config
            .mcp
            .iter()
            .map(|f| tool_config.resolve_path(f))
            .collect(),
        _ => unreachable!("Invalid file_type: {}", file_type),
    };

    if files_to_open.is_empty() {
        anyhow::bail!("No {} files configured for {}", file_type, tool_name);
    }

    let file = pick_file(&files_to_open)?;
    println!("\nOpening: {}", paths::contract_tilde(file));
    editor::open_files(ed, &[file])?;

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
        Commands::Status => status::status(),
        Commands::Link { target, yes } => {
            let config = config::Config::load_from(cli.config.clone())?;
            let central_skills = paths::expand_tilde(&config.central.skills_source);
            let central_prompt = paths::expand_tilde(&config.central.prompt_source);
            let source_dir = paths::expand_tilde(&config.central.source_dir);
            let files_base = paths::expand_tilde(&config.central.files_base);

            let target = match target {
                Some(t) => t,
                None => pick_link_target(&config, "link")?,
            };

            // "central" — only process central files
            if target == "central" {
                if config.central.files.is_empty() {
                    println!("No central files configured.");
                } else {
                    fs::create_dir_all(&files_base)?;
                    println!("{}", "Central files:".bold());
                    for file_path in &config.central.files {
                        let original = paths::expand_path(file_path);
                        files::link_file(&original, &files_base, yes)?;
                    }
                }
                return Ok(());
            }

            // Collect which tools to link
            let tools_to_link: Vec<(&String, &config::ToolConfig)> = if target == "all" {
                config
                    .tools
                    .iter()
                    .filter(|(_, tc)| tc.is_installed())
                    .collect()
            } else {
                let tc = config
                    .tools
                    .get(&target)
                    .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found in config", target))?;
                vec![(
                    config.tools.keys().find(|k| k.as_str() == target).unwrap(),
                    tc,
                )]
            };

            // Prune broken skill links from central store
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

            // Process skill_repos when linking skills
            if !config.central.skill_repos.is_empty() {
                println!("\n{}", "Processing skill repositories...".bold());
                for url in &config.central.skill_repos {
                    match skills::add_from_url(url, &source_dir, &central_skills) {
                        Ok(count) => {
                            if count > 0 {
                                println!("  {} {} skill(s) from {}", " ok ".green(), count, url);
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
                        let resolved_actual =
                            resolved_actual.canonicalize().unwrap_or(resolved_actual);

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
                            )) {
                                let tool_skills_target = source_dir.join("agm_tools").join(key);
                                let added = migrate_skills_dir(
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
                            let resolved_actual =
                                resolved_actual.canonicalize().unwrap_or(resolved_actual);

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

                // Link managed files
                if !tool.files.is_empty() {
                    fs::create_dir_all(&files_base)?;
                    for file_path in &tool.files {
                        let original = tool.resolve_path(file_path);
                        files::link_file(&original, &files_base, yes)?;
                    }
                }
            }

            // Process central-level managed files when linking all tools
            if target == "all" && !config.central.files.is_empty() {
                fs::create_dir_all(&files_base)?;
                println!("\n{}", "Central files:".bold());
                for file_path in &config.central.files {
                    let original = paths::expand_path(file_path);
                    files::link_file(&original, &files_base, yes)?;
                }
            }

            Ok(())
        }
        Commands::Unlink { target } => {
            let config = config::Config::load_from(cli.config.clone())?;
            let central_skills = paths::expand_tilde(&config.central.skills_source);
            let central_prompt = paths::expand_tilde(&config.central.prompt_source);
            let files_base = paths::expand_tilde(&config.central.files_base);

            let target = match target {
                Some(t) => t,
                None => pick_link_target(&config, "unlink")?,
            };

            // "central" — only remove central file links
            if target == "central" {
                if config.central.files.is_empty() {
                    println!("No central files configured.");
                } else {
                    println!("{}", "Central files:".bold());
                    for file_path in &config.central.files {
                        let original = paths::expand_path(file_path);
                        if files::unlink_file(&original, &files_base)? {
                            let central = files::centralized_path(&original, &files_base);
                            if central.exists() {
                                if let Some(parent) = original.parent() {
                                    fs::create_dir_all(parent)?;
                                }
                                fs::copy(&central, &original)?;
                                println!("  {} {} copied back", " ok ".green(), original.display());
                            }
                        }
                    }
                }
                return Ok(());
            }

            // Collect which tools to unlink
            let tools_to_unlink: Vec<(&String, &config::ToolConfig)> = if target == "all" {
                config
                    .tools
                    .iter()
                    .filter(|(_, tc)| tc.is_installed())
                    .collect()
            } else {
                let tc = config
                    .tools
                    .get(&target)
                    .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found in config", target))?;
                vec![(
                    config.tools.keys().find(|k| k.as_str() == target).unwrap(),
                    tc,
                )]
            };

            for (key, tool_config) in tools_to_unlink {
                println!("Unlinking {} ({}):", key, tool_config.name);

                // Remove skills link then copy central skills back
                if !tool_config.skills_dir.is_empty() {
                    let skills_link = tool_config
                        .resolved_config_dir()
                        .join(&tool_config.skills_dir);
                    if linker::remove_link(&skills_link, "skills", true)? && central_skills.is_dir()
                    {
                        copy_dir_all(&central_skills, &skills_link)?;
                        println!("  {} skills copied back", " ok ".green());
                    }
                }

                // Remove prompt link then copy central prompt back
                if !tool_config.prompt_filename.is_empty() {
                    let prompt_link = tool_config
                        .resolved_config_dir()
                        .join(&tool_config.prompt_filename);
                    if linker::remove_link(&prompt_link, "prompt", false)?
                        && central_prompt.exists()
                    {
                        fs::copy(&central_prompt, &prompt_link)?;
                        println!("  {} prompt copied back", " ok ".green());
                    }
                }

                // Remove managed file links then copy central files back
                for file_path in &tool_config.files {
                    let original = tool_config.resolve_path(file_path);
                    if files::unlink_file(&original, &files_base)? {
                        let central = files::centralized_path(&original, &files_base);
                        if central.exists() {
                            if let Some(parent) = original.parent() {
                                fs::create_dir_all(parent)?;
                            }
                            fs::copy(&central, &original)?;
                            println!("  {} {} copied back", " ok ".green(), original.display());
                        }
                    }
                }
            }

            // Remove central-level managed file links (only when "all")
            if target == "all" && !config.central.files.is_empty() {
                println!("\n{}", "Central files:".bold());
                for file_path in &config.central.files {
                    let original = paths::expand_path(file_path);
                    if files::unlink_file(&original, &files_base)? {
                        let central = files::centralized_path(&original, &files_base);
                        if central.exists() {
                            if let Some(parent) = original.parent() {
                                fs::create_dir_all(parent)?;
                            }
                            fs::copy(&central, &original)?;
                            println!("  {} {} copied back", " ok ".green(), original.display());
                        }
                    }
                }
            }

            Ok(())
        }
        Commands::Skills { action } => {
            let mut config = config::Config::load_from(cli.config.clone())?;
            let skills_dir = paths::expand_tilde(&config.central.skills_source);
            let source_dir = paths::expand_tilde(&config.central.source_dir);

            let action = match action {
                Some(a) => a,
                None => {
                    use dialoguer::{theme::ColorfulTheme, Select};
                    let labels = [
                        "list        show all installed skills",
                        "add         install skill(s) from path or URL",
                        "remove      remove a skill",
                        "update      git pull all skill repos",
                    ];
                    let idx = Select::with_theme(&ColorfulTheme::default())
                        .with_prompt("agm skills — select action")
                        .items(&labels)
                        .default(0)
                        .interact()?;
                    match idx {
                        0 => SkillsAction::List,
                        1 => {
                            use dialoguer::Input;
                            let source: String = Input::with_theme(&ColorfulTheme::default())
                                .with_prompt("Path or URL")
                                .interact_text()?;
                            SkillsAction::Add { source }
                        }
                        2 => {
                            use dialoguer::Input;
                            let name: String = Input::with_theme(&ColorfulTheme::default())
                                .with_prompt("Skill name")
                                .interact_text()?;
                            SkillsAction::Remove { name }
                        }
                        _ => SkillsAction::Update,
                    }
                }
            };

            match action {
                SkillsAction::List => {
                    let skills = skills::list_skills(&skills_dir)?;
                    if skills.is_empty() {
                        println!("No skills installed.");
                    } else {
                        println!("\n{} skills installed:", skills.len());
                        for (name, source) in skills {
                            println!("  {} → {}", name, paths::contract_tilde(&source));
                        }
                    }
                    Ok(())
                }
                SkillsAction::Add { source } => {
                    if skills::is_url(&source) {
                        let added = skills::add_from_url(&source, &source_dir, &skills_dir)?;
                        config.add_skill_repo(&source)?;
                        println!("\n{} skill(s) added from URL.", added);
                    } else {
                        let source_path = paths::expand_tilde(&source);
                        println!(
                            "Adding skills from {}...",
                            paths::contract_tilde(&source_path)
                        );
                        let added = skills::add_local(&source_path, &skills_dir)?;
                        println!("\n{} skill(s) added.", added);
                    }
                    Ok(())
                }
                SkillsAction::Remove { name } => {
                    skills::remove_skill(&name, &skills_dir)?;
                    Ok(())
                }
                SkillsAction::Update => {
                    skills::update_all(&skills_dir)?;
                    Ok(())
                }
            }
        }
        Commands::Prompt { target } => {
            let config = config::Config::load_from(cli.config.clone())?;
            let ed = editor::get_editor(&config);
            let target = match target {
                Some(t) => t,
                None => pick_target(&config, "prompt", true)?,
            };
            match target.as_str() {
                "central" => {
                    let p = paths::expand_tilde(&config.central.prompt_source);
                    println!("\nOpening: {}", paths::contract_tilde(&p));
                    editor::open_files(&ed, &[&p])?;
                }
                tool_name => open_tool_files(&config, &ed, tool_name, "prompt")?,
            }
            Ok(())
        }
        Commands::Config { target } => {
            let config = config::Config::load_from(cli.config.clone())?;
            let ed = editor::get_editor(&config);
            let target = match target {
                Some(t) => t,
                None => pick_target(&config, "config", true)?,
            };
            match target.as_str() {
                "central" => {
                    let p = cli
                        .config
                        .clone()
                        .unwrap_or_else(config::Config::config_path);
                    println!("\nOpening: {}", paths::contract_tilde(&p));
                    editor::open_files(&ed, &[&p])?;
                }
                tool_name => open_tool_files(&config, &ed, tool_name, "config")?,
            }
            Ok(())
        }
        Commands::Auth { target } => {
            let config = config::Config::load_from(cli.config.clone())?;
            let ed = editor::get_editor(&config);
            let target = match target {
                Some(t) => t,
                None => pick_target(&config, "auth", false)?,
            };
            open_tool_files(&config, &ed, &target, "auth")?;
            Ok(())
        }
        Commands::Mcp { target } => {
            let config = config::Config::load_from(cli.config.clone())?;
            let ed = editor::get_editor(&config);
            let target = match target {
                Some(t) => t,
                None => pick_target(&config, "mcp", false)?,
            };
            open_tool_files(&config, &ed, &target, "mcp")?;
            Ok(())
        }
    }
}
