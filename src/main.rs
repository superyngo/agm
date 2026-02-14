mod config;
mod editor;
mod init;
mod linker;
mod paths;
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

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize agm config and central directories
    Init,
    /// Show link status for all tools
    Status,
    /// List all registered tools
    List,
    /// Verify all symlinks are healthy
    Check,
    /// Create/repair symlinks
    Link {
        /// Only link "skills" or "prompts" (default: both)
        target: Option<String>,
        /// Skip all confirmation prompts
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
    /// Remove symlinks for a tool
    Unlink {
        /// Tool name (e.g. claude, gemini)
        tool: String,
    },
    /// Manage skills
    Skills {
        #[command(subcommand)]
        action: Option<SkillsAction>,
    },
    /// Open config files in editor
    Edit {
        /// File type: "prompt", "config", "auth", "mcp"
        file_type: String,
        /// Optional tool name (claude, gemini, copilot, etc.)
        tool: Option<String>,
    },
}

#[derive(Subcommand)]
enum SkillsAction {
    /// Install skill(s) from local path or repo URL
    Add { source: String },
    /// Remove a skill
    Remove { name: String },
    /// Git pull all skill source repos
    Update,
}

fn select_installed_tool(config: &config::Config) -> anyhow::Result<String> {
    let installed_tools: Vec<_> = config
        .tools
        .iter()
        .filter(|(_, tool)| tool.is_installed())
        .collect();

    if installed_tools.is_empty() {
        anyhow::bail!("No tools are installed");
    }

    println!("Select a tool:");
    for (i, (key, tool)) in installed_tools.iter().enumerate() {
        println!("  {}) {} ({})", i + 1, key, tool.name);
    }

    print!("\nEnter number (or q to quit): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input == "q" || input == "Q" {
        std::process::exit(0);
    }

    let index: usize = input
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid input: please enter a number"))?;

    installed_tools
        .get(index.saturating_sub(1))
        .map(|(key, _)| key.to_string())
        .ok_or_else(|| anyhow::anyhow!("Invalid selection: number out of range"))
}

fn prompt_yes_no(prompt: &str) -> bool {
    print!("{} [y/N]: ", prompt);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().to_lowercase();

    input == "y" || input == "yes"
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
            .map(|f| base_dir.join(f))
            .collect(),
        "auth" => tool_config.auth.iter().map(|f| base_dir.join(f)).collect(),
        "mcp" => tool_config.mcp.iter().map(|f| base_dir.join(f)).collect(),
        _ => unreachable!("Invalid file_type: {}", file_type),
    };

    if files_to_open.is_empty() {
        anyhow::bail!("No {} files configured for {}", file_type, tool_name);
    }

    println!("\nOpening file(s):");
    for path in &files_to_open {
        println!("  {}", paths::contract_tilde(path));
    }
    println!();

    let file_refs: Vec<&Path> = files_to_open.iter().map(|p| p.as_path()).collect();
    editor::open_files(ed, &file_refs)?;

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
        Commands::Init => init::run(),
        Commands::Status => status::status(),
        Commands::List => status::list(),
        Commands::Check => status::check(),
        Commands::Link { target, yes } => {
            let config = config::Config::load()?;
            let central_skills = paths::expand_tilde(&config.central.skills_source);
            let central_prompt = paths::expand_tilde(&config.central.prompt_source);
            let source_dir = paths::expand_tilde(&config.central.source_dir);

            let link_skills = target.as_ref().is_none_or(|t| t == "skills");
            let link_prompts = target.as_ref().is_none_or(|t| t == "prompts");

            // Process skill_repos if target is None or "skills"
            if link_skills && !config.central.skill_repos.is_empty() {
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
            for (key, tool) in &config.tools {
                if !tool.is_installed() {
                    continue;
                }

                println!("\n{} ({}):", key, tool.name);

                // Link skills directory
                if link_skills && !tool.skills_dir.is_empty() {
                    let skills_link = tool.resolved_config_dir().join(&tool.skills_dir);

                    // Check if existing symlink points to wrong target
                    if skills_link.is_symlink() {
                        let actual_target = fs::read_link(&skills_link)?;
                        let expected_target = central_skills
                            .canonicalize()
                            .unwrap_or_else(|_| central_skills.clone());
                        let resolved_actual = skills_link
                            .parent()
                            .map(|p| p.join(&actual_target))
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
                                fs::remove_file(&skills_link)?;
                                println!("  {} Removed old symlink", " ok ".green());
                            } else {
                                println!("  {} Skipping skills link", "skip".yellow());
                                continue;
                            }
                        }
                    }
                    // Check if skills directory exists and has content
                    else if skills_link.is_dir() {
                        let skills_content = skills::scan_skills(&skills_link);
                        if !skills_content.is_empty() {
                            if yes || prompt_yes_no(&format!(
                                "Found {} existing skill(s) in {}. Migrate to AGM and create symlink?",
                                skills_content.len(),
                                paths::contract_tilde(&skills_link)
                            )) {
                                // Create migration target directory in source/skills
                                let tool_skills_target = source_dir.join("skills").join(key);
                                fs::create_dir_all(&tool_skills_target)?;

                                // Move skills directory content to source/skills
                                for entry in fs::read_dir(&skills_link)? {
                                    let entry = entry?;
                                    let from = entry.path();
                                    let to = tool_skills_target.join(from.file_name().unwrap());
                                    fs::rename(&from, &to)?;
                                }

                                // Remove original skills directory
                                fs::remove_dir(&skills_link)?;

                                // Add skills from migrated location
                                let added = skills::add_local(&tool_skills_target, &central_skills)?;
                                if added > 0 {
                                    println!("  {} Migrated and added {} skill(s)", " ok ".green(), added);
                                }
                            } else {
                                println!("  {} Skipping skills migration", "skip".yellow());
                                continue;
                            }
                        }
                    }

                    linker::create_link(&skills_link, &central_skills, "skills")?;
                }

                // Link prompt file
                if link_prompts && !tool.prompt_filename.is_empty() {
                    let prompt_link = tool.resolved_config_dir().join(&tool.prompt_filename);

                    // Check if prompt file exists and is not a symlink
                    if prompt_link.exists() && !prompt_link.is_symlink() {
                        // Check if file is not empty
                        let content = fs::read_to_string(&prompt_link)?;
                        if !content.trim().is_empty() {
                            if yes
                                || prompt_yes_no(&format!(
                                    "Existing prompt file found at {}. Backup and create symlink?",
                                    paths::contract_tilde(&prompt_link)
                                ))
                            {
                                // Create backup filename with timestamp
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
                        }
                    }

                    linker::create_link(&prompt_link, &central_prompt, "prompt")?;
                }
            }

            Ok(())
        }
        Commands::Unlink { tool } => {
            let config = config::Config::load()?;
            let tool_config = config
                .tools
                .get(&tool)
                .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found in config", tool))?;

            println!("Unlinking {} ({}):", tool, tool_config.name);

            // Remove skills symlink
            if !tool_config.skills_dir.is_empty() {
                let skills_link = tool_config
                    .resolved_config_dir()
                    .join(&tool_config.skills_dir);
                linker::remove_link(&skills_link, "skills")?;
            }

            // Remove prompt symlink
            if !tool_config.prompt_filename.is_empty() {
                let prompt_link = tool_config
                    .resolved_config_dir()
                    .join(&tool_config.prompt_filename);
                linker::remove_link(&prompt_link, "prompt")?;
            }

            Ok(())
        }
        Commands::Skills { action } => {
            let mut config = config::Config::load()?;
            let skills_dir = paths::expand_tilde(&config.central.skills_source);
            let source_dir = paths::expand_tilde(&config.central.source_dir);

            match action {
                None => {
                    // List skills
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
                Some(SkillsAction::Add { source }) => {
                    if skills::is_url(&source) {
                        // URL mode
                        let added = skills::add_from_url(&source, &source_dir, &skills_dir)?;
                        config.add_skill_repo(&source)?;
                        println!("\n{} skill(s) added from URL.", added);
                    } else {
                        // Local path mode
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
                Some(SkillsAction::Remove { name }) => {
                    skills::remove_skill(&name, &skills_dir)?;
                    Ok(())
                }
                Some(SkillsAction::Update) => {
                    skills::update_all(&skills_dir)?;
                    Ok(())
                }
            }
        }
        Commands::Edit { file_type, tool } => {
            let config = config::Config::load()?;
            let ed = editor::get_editor(&config);

            match (file_type.as_str(), tool) {
                // No tool specified - open master files or show selection
                ("prompt", None) => {
                    let prompt_path = paths::expand_tilde(&config.central.prompt_source);
                    println!("\nOpening file(s):");
                    println!("  {}", paths::contract_tilde(&prompt_path));
                    println!();
                    editor::open_files(&ed, &[&prompt_path])?;
                }
                ("config", None) => {
                    let config_path = config::Config::config_path();
                    println!("\nOpening file(s):");
                    println!("  {}", paths::contract_tilde(&config_path));
                    println!();
                    editor::open_files(&ed, &[&config_path])?;
                }
                ("auth", None) | ("mcp", None) => {
                    let selected_tool = select_installed_tool(&config)?;
                    open_tool_files(&config, &ed, &selected_tool, file_type.as_str())?;
                }

                // Tool specified - open tool-specific files
                ("prompt", Some(tool_name))
                | ("config", Some(tool_name))
                | ("auth", Some(tool_name))
                | ("mcp", Some(tool_name)) => {
                    open_tool_files(&config, &ed, &tool_name, file_type.as_str())?;
                }

                // Invalid file type
                (invalid, _) => {
                    anyhow::bail!(
                        "Invalid file type: '{}'. Use: prompt, config, auth, or mcp",
                        invalid
                    );
                }
            }

            Ok(())
        }
    }
}
