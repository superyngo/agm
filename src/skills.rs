use anyhow::Context;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::contract_tilde;
use crate::platform;

/// Installation status of a skill in the central skills directory
#[derive(Debug, Clone, PartialEq)]
pub enum SkillInstallStatus {
    /// Central skills dir has a symlink pointing to this skill's source
    Installed,
    /// Source exists but no central link
    NotInstalled,
    /// Another skill with the same name is installed from a different source
    Conflict,
}

/// Full info about a single skill
#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub source_path: PathBuf,
    pub install_status: SkillInstallStatus,
}

/// Full info about a single agent (.md file)
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub name: String,
    pub source_path: PathBuf,
    pub install_status: SkillInstallStatus,
}

/// Full info about a single command (.md file in commands/ folder)
#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub name: String,
    pub source_path: PathBuf,
    pub install_status: SkillInstallStatus,
}

/// What kind of source this is
#[derive(Debug, Clone)]
pub enum SourceKind {
    /// Git-cloned repository (URL from config or git remote lookup)
    Repo { url: Option<String> },
    /// Copied local directory (source_dir/local/{name}/)
    Local,
    /// Migrated from tool during agm link (source_dir/agm_tools/{tool}/)
    Migrated { tool: String },
}

/// A source and all skills/agents it contains
#[derive(Debug, Clone)]
pub struct SourceGroup {
    pub name: String,
    pub kind: SourceKind,
    pub path: PathBuf,
    pub skills: Vec<SkillInfo>,
    pub agents: Vec<AgentInfo>,
    pub commands: Vec<CommandInfo>,
}

/// Progress report from update_all_with_progress
#[derive(Debug, Clone)]
pub enum UpdateProgress {
    RepoStart {
        name: String,
    },
    RepoComplete {
        name: String,
        success: bool,
        message: String,
    },
    AllDone {
        total: usize,
        updated: usize,
        new_skills: usize,
        new_agents: usize,
        new_commands: usize,
    },
}

/// Scan a path for skills. Returns list of (skill_name, skill_dir_path).
/// If path/SKILL.md exists → single skill.
/// Else scan subdirectories recursively (max depth 3) for SKILL.md.
pub fn scan_skills(path: &Path) -> Vec<(String, PathBuf)> {
    let mut skills = Vec::new();

    // Check if this is a single skill
    if path.join("SKILL.md").exists() {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            skills.push((name.to_string(), path.to_path_buf()));
        }
        return skills;
    }

    // Recursively scan for skills (max depth 3)
    scan_skills_recursive(path, &mut skills, 0, 3);
    skills
}

/// Scan the `agents/` directory within a source path for `.md` files.
/// Returns list of (agent_name_without_ext, full_path_to_md).
pub fn scan_agents(path: &Path) -> Vec<(String, PathBuf)> {
    let agents_dir = path.join("agents");
    if !agents_dir.is_dir() {
        return vec![];
    }
    let mut agents = Vec::new();
    let Ok(entries) = fs::read_dir(&agents_dir) else {
        return agents;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            if let Some(ext) = p.extension() {
                if ext == "md" {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        agents.push((stem.to_string(), p));
                    }
                }
            }
        }
    }
    agents.sort_by(|a, b| a.0.cmp(&b.0));
    agents
}

pub fn scan_commands(path: &Path) -> Vec<(String, PathBuf)> {
    let commands_dir = path.join("commands");
    if !commands_dir.is_dir() {
        return vec![];
    }
    let mut commands = Vec::new();
    let Ok(entries) = fs::read_dir(&commands_dir) else {
        return commands;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            if let Some(ext) = p.extension() {
                if ext == "md" {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        commands.push((stem.to_string(), p));
                    }
                }
            }
        }
    }
    commands.sort_by(|a, b| a.0.cmp(&b.0));
    commands
}

fn scan_skills_recursive(
    dir: &Path,
    skills: &mut Vec<(String, PathBuf)>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.join("SKILL.md").exists() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    skills.push((name.to_string(), path.clone()));
                }
            } else {
                // Recurse into subdirectories
                scan_skills_recursive(&path, skills, depth + 1, max_depth);
            }
        }
    }
}

/// Install a single skill by creating a symlink in the central skills directory.
/// Errors if a skill with the same name from a different source already exists.
pub fn install_skill(name: &str, source_path: &Path, skills_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(skills_dir)?;
    let link_path = skills_dir.join(name);

    if link_path.exists() || link_path.symlink_metadata().is_ok() {
        if platform::is_dir_link(&link_path) {
            if let Some(target) = platform::read_dir_link_target(&link_path) {
                let target_canon = fs::canonicalize(&target).unwrap_or(target);
                let source_canon =
                    fs::canonicalize(source_path).unwrap_or(source_path.to_path_buf());
                if target_canon == source_canon {
                    return Ok(());
                }
            }
        }
        anyhow::bail!(
            "Skill '{}' already exists (installed from another source). Uninstall it first.",
            name
        );
    }

    platform::link_dir(source_path, &link_path)
        .with_context(|| format!("Failed to install skill: {}", name))?;
    Ok(())
}

/// Uninstall a single skill by removing its symlink from the central skills directory.
/// No-op if the skill is not installed. Source directory is NOT deleted.
pub fn uninstall_skill(name: &str, skills_dir: &Path) -> anyhow::Result<()> {
    let link_path = skills_dir.join(name);
    if link_path.symlink_metadata().is_err() {
        return Ok(());
    }
    if platform::is_dir_link(&link_path) {
        platform::remove_link(&link_path)?;
    }
    Ok(())
}

/// Install a single agent by creating a file symlink in the central agents directory.
pub fn install_agent(name: &str, source_path: &Path, agents_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(agents_dir)?;
    let link_name = format!("{}.md", name);
    let link_path = agents_dir.join(&link_name);

    if link_path.exists() || link_path.symlink_metadata().is_ok() {
        // Check if it already points to same source
        if let Ok(target) = fs::read_link(&link_path) {
            let target_canon = fs::canonicalize(&target).unwrap_or(target);
            let source_canon = fs::canonicalize(source_path).unwrap_or(source_path.to_path_buf());
            if target_canon == source_canon {
                return Ok(());
            }
        }
        anyhow::bail!(
            "Agent '{}' already exists (installed from another source). Uninstall it first.",
            name
        );
    }

    platform::link_file(source_path, &link_path)
        .with_context(|| format!("Failed to install agent: {}", name))?;
    Ok(())
}

/// Install a single command by symlinking its .md file into the central commands directory.
pub fn install_command(name: &str, source_path: &Path, commands_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(commands_dir)?;
    let link_name = format!("{}.md", name);
    let link_path = commands_dir.join(&link_name);

    if link_path.exists() || link_path.symlink_metadata().is_ok() {
        if let Ok(target) = fs::read_link(&link_path) {
            let target_canon = fs::canonicalize(&target).unwrap_or(target);
            let source_canon = fs::canonicalize(source_path).unwrap_or(source_path.to_path_buf());
            if target_canon == source_canon {
                return Ok(());
            }
        }
        anyhow::bail!(
            "Command '{}' already exists (installed from another source). Uninstall it first.",
            name
        );
    }

    platform::link_file(source_path, &link_path)
        .with_context(|| format!("Failed to install command: {}", name))?;
    Ok(())
}

/// Uninstall a single agent by removing its symlink from the central agents directory.
pub fn uninstall_agent(name: &str, agents_dir: &Path) -> anyhow::Result<()> {
    let link_name = format!("{}.md", name);
    let link_path = agents_dir.join(&link_name);
    if link_path.symlink_metadata().is_err() {
        return Ok(());
    }
    platform::remove_link(&link_path)?;
    Ok(())
}

/// Uninstall a single command by removing its symlink from the central commands directory.
pub fn uninstall_command(name: &str, commands_dir: &Path) -> anyhow::Result<()> {
    let link_name = format!("{}.md", name);
    let link_path = commands_dir.join(&link_name);
    if link_path.symlink_metadata().is_err() {
        return Ok(());
    }
    platform::remove_link(&link_path)?;
    Ok(())
}

/// Scan central skills directory and remove any symlinks whose targets no longer exist.
/// Returns the number of broken links removed.
pub fn prune_broken_skills(skills_dir: &Path) -> anyhow::Result<usize> {
    if !skills_dir.is_dir() {
        return Ok(0);
    }

    let mut removed = 0;
    for entry in fs::read_dir(skills_dir)? {
        let entry = entry?;
        let path = entry.path();
        if platform::is_dir_link(&path) {
            // Follow the link; if target doesn't exist the link is broken
            if !path.exists() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("<unknown>");
                platform::remove_link(&path)?;
                println!("  {} {} (broken skill link removed)", "warn".yellow(), name);
                removed += 1;
            }
        }
    }
    Ok(removed)
}

/// Scan central agents directory and remove any symlinks whose targets no longer exist.
pub fn prune_broken_agents(agents_dir: &Path) -> anyhow::Result<usize> {
    if !agents_dir.is_dir() {
        return Ok(0);
    }

    let mut removed = 0;
    for entry in fs::read_dir(agents_dir)? {
        let entry = entry?;
        let path = entry.path();
        // Only consider .md files that are symlinks
        if path.extension().and_then(|e| e.to_str()) == Some("md")
            && path.symlink_metadata().is_ok()
            && !path.exists()
        {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("<unknown>");
            platform::remove_link(&path)?;
            println!("  {} {} (broken agent link removed)", "warn".yellow(), name);
            removed += 1;
        }
    }
    Ok(removed)
}

/// Scan central commands directory and remove any symlinks whose targets no longer exist.
pub fn prune_broken_commands(commands_dir: &Path) -> anyhow::Result<usize> {
    if !commands_dir.is_dir() {
        return Ok(0);
    }

    let mut removed = 0;
    for entry in fs::read_dir(commands_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md")
            && path.symlink_metadata().is_ok()
            && !path.exists()
        {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("<unknown>");
            platform::remove_link(&path)?;
            println!("  {} {} (broken command link removed)", "warn".yellow(), name);
            removed += 1;
        }
    }
    Ok(removed)
}

/// Check the install status of an agent by examining the central agents directory.
fn check_agent_install_status(
    name: &str,
    source_path: &Path,
    agents_dir: &Path,
) -> SkillInstallStatus {
    let link_name = format!("{}.md", name);
    let link_path = agents_dir.join(&link_name);
    if link_path.symlink_metadata().is_err() {
        return SkillInstallStatus::NotInstalled;
    }
    if let Ok(target) = fs::read_link(&link_path) {
        let target_canon = fs::canonicalize(&target).unwrap_or(target);
        let source_canon = fs::canonicalize(source_path).unwrap_or(source_path.to_path_buf());
        if target_canon == source_canon {
            return SkillInstallStatus::Installed;
        }
    }
    SkillInstallStatus::Conflict
}

fn check_command_install_status(
    name: &str,
    source_path: &Path,
    commands_dir: &Path,
) -> SkillInstallStatus {
    let link_name = format!("{}.md", name);
    let link_path = commands_dir.join(&link_name);
    if link_path.symlink_metadata().is_err() {
        return SkillInstallStatus::NotInstalled;
    }
    if let Ok(target) = fs::read_link(&link_path) {
        let target_canon = fs::canonicalize(&target).unwrap_or(target);
        let source_canon = fs::canonicalize(source_path).unwrap_or(source_path.to_path_buf());
        if target_canon == source_canon {
            return SkillInstallStatus::Installed;
        }
    }
    SkillInstallStatus::Conflict
}

pub fn is_url(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://") || source.starts_with("git@")
}

/// Derive repo name from URL
pub fn repo_name_from_url(url: &str) -> String {
    // e.g. "https://github.com/user/my-skills.git" → "my-skills"
    url.rsplit('/')
        .next()
        .unwrap_or("repo")
        .trim_end_matches(".git")
        .to_string()
}

/// Normalize a git URL for comparison (strip trailing .git, convert SSH to path form)
fn normalize_git_url(url: &str) -> String {
    let s = url.trim().trim_end_matches('/').trim_end_matches(".git");
    // Convert git@host:user/repo to host/user/repo for comparison
    if let Some(rest) = s.strip_prefix("git@") {
        rest.replacen(':', "/", 1).to_lowercase()
    } else {
        s.to_lowercase()
    }
}

/// Git pull all skill source repos (deduplicating by git root), then re-sync symlinks
pub fn update_all(skills_dir: &Path, agents_dir: &Path, source_dir: &Path) -> anyhow::Result<()> {
    if !skills_dir.is_dir() {
        anyhow::bail!("Skills directory does not exist: {}", skills_dir.display());
    }

    // Collect git roots from source_dir (not from skills symlinks)
    let mut git_roots = std::collections::HashSet::new();
    if source_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(source_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                // Skip local/ and agm_tools/ — they aren't git repos
                if name == "local" || name == "agm_tools" {
                    continue;
                }
                if path.is_dir() && path.join(".git").exists() {
                    git_roots.insert(path);
                }
            }
        }
    }

    if git_roots.is_empty() {
        println!("No git repositories found in source directory.");
        return Ok(());
    }

    println!("Updating {} git repositories...\n", git_roots.len());

    for git_root in &git_roots {
        println!("Updating {}...", contract_tilde(git_root));
        let status = std::process::Command::new("git")
            .args(["pull"])
            .current_dir(git_root)
            .status()?;

        if status.success() {
            println!("{} Updated {}\n", " ok ".green(), contract_tilde(git_root));
        } else {
            println!(
                "{} Failed to update {}\n",
                "fail".red(),
                contract_tilde(git_root)
            );
        }
    }

    // Prune broken links (consistent with list/manage behavior)
    println!("{}", "Syncing central skills symlinks...".bold());
    let pruned = prune_broken_skills(skills_dir)?;
    if pruned > 0 {
        println!(
            "  {} Removed {} broken skill link(s)",
            "warn".yellow(),
            pruned
        );
    }
    let pruned_agents = prune_broken_agents(agents_dir)?;
    if pruned_agents > 0 {
        println!(
            "  {} Removed {} broken agent link(s)",
            "warn".yellow(),
            pruned_agents
        );
    }

    // Re-sync: for each repo, find new skills/agents not yet installed
    for git_root in &git_roots {
        let new_skills = scan_skills(git_root);
        let mut added = 0;
        for (name, skill_path) in new_skills {
            let link_path = skills_dir.join(&name);
            if link_path.symlink_metadata().is_err() {
                if let Err(e) = install_skill(&name, &skill_path, skills_dir) {
                    println!("  {} {}: {}", "warn".yellow(), name, e);
                } else {
                    println!(
                        "  {} {} → {}",
                        " ok ".green(),
                        name,
                        contract_tilde(&skill_path)
                    );
                    added += 1;
                }
            }
        }

        let new_agents = scan_agents(git_root);
        let mut agents_added = 0;
        for (name, agent_path) in new_agents {
            let link_name = format!("{}.md", name);
            let link_path = agents_dir.join(&link_name);
            if link_path.symlink_metadata().is_err() {
                if let Err(e) = install_agent(&name, &agent_path, agents_dir) {
                    println!("  {} agent {}: {}", "warn".yellow(), name, e);
                } else {
                    println!(
                        "  {} agent {} → {}",
                        " ok ".green(),
                        name,
                        contract_tilde(&agent_path)
                    );
                    agents_added += 1;
                }
            }
        }

        if added > 0 || agents_added > 0 {
            println!(
                "  {} {} new skill(s), {} new agent(s) from {}",
                " ok ".green(),
                added,
                agents_added,
                contract_tilde(git_root)
            );
        }
    }

    Ok(())
}

/// Like update_all, but reports progress through a callback.
/// Used by the TUI for non-blocking background updates.
pub fn update_all_with_progress<F>(
    skills_dir: &Path,
    agents_dir: &Path,
    commands_dir: &Path,
    source_dir: &Path,
    mut on_progress: F,
) where
    F: FnMut(UpdateProgress),
{
    // Collect git roots (same logic as update_all)
    let mut git_roots = std::collections::HashSet::new();
    if source_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(source_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name == "local" || name == "agm_tools" {
                    continue;
                }
                if path.is_dir() && path.join(".git").exists() {
                    git_roots.insert(path);
                }
            }
        }
    }

    let total = git_roots.len();
    let mut updated = 0;
    let mut new_skills_total = 0;
    let mut new_agents_total = 0;
    let mut new_commands_total = 0;

    if total == 0 {
        on_progress(UpdateProgress::AllDone {
            total: 0,
            updated: 0,
            new_skills: 0,
            new_agents: 0,
            new_commands: 0,
        });
        return;
    }

    // Git pull each repo
    for git_root in &git_roots {
        let repo_name = git_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        on_progress(UpdateProgress::RepoStart {
            name: repo_name.clone(),
        });

        let result = std::process::Command::new("git")
            .args(["pull"])
            .current_dir(git_root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();

        match result {
            Ok(output) if output.status.success() => {
                let msg = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let message = if msg.contains("Already up to date") {
                    "Already up to date".to_string()
                } else {
                    "Updated".to_string()
                };
                on_progress(UpdateProgress::RepoComplete {
                    name: repo_name,
                    success: true,
                    message,
                });
                updated += 1;
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                on_progress(UpdateProgress::RepoComplete {
                    name: repo_name,
                    success: false,
                    message: format!("Failed: {}", stderr),
                });
            }
            Err(e) => {
                on_progress(UpdateProgress::RepoComplete {
                    name: repo_name,
                    success: false,
                    message: format!("Error: {}", e),
                });
            }
        }
    }

    // Prune broken links (silently — TUI will show in log if needed)
    let _ = prune_broken_skills(skills_dir);
    let _ = prune_broken_agents(agents_dir);
    let _ = prune_broken_commands(commands_dir);

    // Re-sync new skills/agents
    for git_root in &git_roots {
        let new_skills = scan_skills(git_root);
        for (name, skill_path) in new_skills {
            let link_path = skills_dir.join(&name);
            if link_path.symlink_metadata().is_err()
                && install_skill(&name, &skill_path, skills_dir).is_ok()
            {
                new_skills_total += 1;
            }
        }

        let new_agents = scan_agents(git_root);
        for (name, agent_path) in new_agents {
            let link_name = format!("{}.md", name);
            let link_path = agents_dir.join(&link_name);
            if link_path.symlink_metadata().is_err()
                && install_agent(&name, &agent_path, agents_dir).is_ok()
            {
                new_agents_total += 1;
            }
        }

        let new_cmds = scan_commands(git_root);
        for (name, cmd_path) in new_cmds {
            let link_name = format!("{}.md", name);
            let link_path = commands_dir.join(&link_name);
            if link_path.symlink_metadata().is_err()
                && install_command(&name, &cmd_path, commands_dir).is_ok()
            {
                new_commands_total += 1;
            }
        }
    }

    on_progress(UpdateProgress::AllDone {
        total,
        updated,
        new_skills: new_skills_total,
        new_agents: new_agents_total,
        new_commands: new_commands_total,
    });
}

/// Try to resolve the git remote URL for a directory.
fn resolve_repo_url(dir_name: &str, path: &Path, source_repos: &[String]) -> Option<String> {
    for url in source_repos {
        if repo_name_from_url(url) == dir_name {
            return Some(url.clone());
        }
    }
    std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(path)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

/// Check the install status of a skill by examining the central skills directory.
fn check_install_status(name: &str, source_path: &Path, skills_dir: &Path) -> SkillInstallStatus {
    let link_path = skills_dir.join(name);
    if link_path.symlink_metadata().is_err() {
        return SkillInstallStatus::NotInstalled;
    }
    if platform::is_dir_link(&link_path) {
        if let Some(target) = platform::read_dir_link_target(&link_path) {
            let target_canon = fs::canonicalize(&target).unwrap_or(target);
            let source_canon = fs::canonicalize(source_path).unwrap_or(source_path.to_path_buf());
            if target_canon == source_canon {
                return SkillInstallStatus::Installed;
            }
        }
    }
    SkillInstallStatus::Conflict
}

/// Scan the source directory and return all sources grouped with their skills/agents and install status.
/// Gracefully returns empty Vec if source_dir doesn't exist.
pub fn scan_all_sources(
    source_dir: &Path,
    skills_dir: &Path,
    agents_dir: &Path,
    commands_dir: &Path,
    source_repos: &[String],
) -> Vec<SourceGroup> {
    if !source_dir.is_dir() {
        return vec![];
    }

    let mut groups = Vec::new();

    let entries: Vec<_> = match fs::read_dir(source_dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return vec![],
    };

    for entry in entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if dir_name == "local" {
            if let Ok(sub_entries) = fs::read_dir(&path) {
                for sub_entry in sub_entries.filter_map(|e| e.ok()) {
                    let sub_path = sub_entry.path();
                    if !sub_path.is_dir() {
                        continue;
                    }
                    let sub_name = match sub_path.file_name().and_then(|n| n.to_str()) {
                        Some(n) => n.to_string(),
                        None => continue,
                    };
                    let skills = scan_skills(&sub_path)
                        .into_iter()
                        .map(|(name, sp)| SkillInfo {
                            install_status: check_install_status(&name, &sp, skills_dir),
                            name,
                            source_path: sp,
                        })
                        .collect();
                    let agents = scan_agents(&sub_path)
                        .into_iter()
                        .map(|(name, sp)| AgentInfo {
                            install_status: check_agent_install_status(&name, &sp, agents_dir),
                            name,
                            source_path: sp,
                        })
                        .collect();
                    let commands = scan_commands(&sub_path)
                        .into_iter()
                        .map(|(name, sp)| CommandInfo {
                            install_status: check_command_install_status(&name, &sp, commands_dir),
                            name,
                            source_path: sp,
                        })
                        .collect();
                    groups.push(SourceGroup {
                        name: sub_name,
                        kind: SourceKind::Local,
                        path: sub_path,
                        skills,
                        agents,
                        commands,
                    });
                }
            }
        } else if dir_name == "agm_tools" {
            if let Ok(sub_entries) = fs::read_dir(&path) {
                for sub_entry in sub_entries.filter_map(|e| e.ok()) {
                    let sub_path = sub_entry.path();
                    if !sub_path.is_dir() {
                        continue;
                    }
                    let tool_name = match sub_path.file_name().and_then(|n| n.to_str()) {
                        Some(n) => n.to_string(),
                        None => continue,
                    };
                    let skills = scan_skills(&sub_path)
                        .into_iter()
                        .map(|(name, sp)| SkillInfo {
                            install_status: check_install_status(&name, &sp, skills_dir),
                            name,
                            source_path: sp,
                        })
                        .collect();
                    let agents = scan_agents(&sub_path)
                        .into_iter()
                        .map(|(name, sp)| AgentInfo {
                            install_status: check_agent_install_status(&name, &sp, agents_dir),
                            name,
                            source_path: sp,
                        })
                        .collect();
                    let commands = scan_commands(&sub_path)
                        .into_iter()
                        .map(|(name, sp)| CommandInfo {
                            install_status: check_command_install_status(&name, &sp, commands_dir),
                            name,
                            source_path: sp,
                        })
                        .collect();
                    groups.push(SourceGroup {
                        name: format!("agm_tools/{}", tool_name),
                        kind: SourceKind::Migrated { tool: tool_name },
                        path: sub_path,
                        skills,
                        agents,
                        commands,
                    });
                }
            }
        } else {
            let url = resolve_repo_url(&dir_name, &path, source_repos);
            let skills = scan_skills(&path)
                .into_iter()
                .map(|(name, sp)| SkillInfo {
                    install_status: check_install_status(&name, &sp, skills_dir),
                    name,
                    source_path: sp,
                })
                .collect();
            let agents = scan_agents(&path)
                .into_iter()
                .map(|(name, sp)| AgentInfo {
                    install_status: check_agent_install_status(&name, &sp, agents_dir),
                    name,
                    source_path: sp,
                })
                .collect();
            let commands = scan_commands(&path)
                .into_iter()
                .map(|(name, sp)| CommandInfo {
                    install_status: check_command_install_status(&name, &sp, commands_dir),
                    name,
                    source_path: sp,
                })
                .collect();
            groups.push(SourceGroup {
                name: dir_name,
                kind: SourceKind::Repo { url },
                path,
                skills,
                agents,
                commands,
            });
        }
    }

    groups.sort_by(|a, b| a.name.cmp(&b.name));
    groups
}

/// Copy a local directory into source_dir/local/{name}/ and return the list of skills found.
/// Scans BEFORE copying — errors if no skills found. Original directory preserved.
pub fn add_local_copy(
    source: &Path,
    source_dir: &Path,
) -> anyhow::Result<(PathBuf, Vec<(String, PathBuf)>)> {
    if !source.exists() {
        anyhow::bail!("Source path does not exist: {}", source.display());
    }

    // Scan before copying
    let pre_skills = scan_skills(source);
    if pre_skills.is_empty() {
        anyhow::bail!(
            "No skills found at {}. A skill must contain a SKILL.md file.",
            source.display()
        );
    }

    let source_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed")
        .to_string();

    let dest = source_dir.join("local").join(&source_name);
    if dest.exists() {
        anyhow::bail!(
            "Source '{}' already exists at {}. Remove it first or choose a different name.",
            source_name,
            contract_tilde(&dest)
        );
    }

    // Copy
    fs::create_dir_all(dest.parent().unwrap())?;
    copy_dir_recursive(source, &dest)?;

    // Re-scan the copied location
    let skills = scan_skills(&dest);
    Ok((dest, skills))
}

/// Clone a git repo (or pull if it already exists) into source_dir/{repo_name}/.
/// Returns the repo path and list of skills found. Does NOT install skills.
pub fn clone_or_pull(
    url: &str,
    source_dir: &Path,
) -> anyhow::Result<(PathBuf, Vec<(String, PathBuf)>)> {
    let name = repo_name_from_url(url);
    let repo_path = source_dir.join(&name);

    if repo_path.is_dir() {
        // Check if it belongs to a different remote
        let existing_url = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(&repo_path)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            });

        if let Some(ref existing) = existing_url {
            if normalize_git_url(existing) != normalize_git_url(url) {
                anyhow::bail!(
                    "Directory '{}' already exists but belongs to a different repo ({}).\n\
                     Remove it manually or use a different URL.",
                    name,
                    existing
                );
            }
        }

        println!("Updating {} from {}...", name, url);
        let status = std::process::Command::new("git")
            .args(["pull"])
            .current_dir(&repo_path)
            .status()?;
        if !status.success() {
            anyhow::bail!("git pull failed for {}", name);
        }
    } else {
        println!("Cloning {} from {}...", name, url);
        fs::create_dir_all(source_dir)?;
        let status = std::process::Command::new("git")
            .args(["clone", url, &repo_path.display().to_string()])
            .status()?;
        if !status.success() {
            anyhow::bail!("git clone failed for {}", url);
        }
    }

    let skills = scan_skills(&repo_path);
    if skills.is_empty() {
        fs::remove_dir_all(&repo_path)?;
        anyhow::bail!("No skills found in {}. Clone removed.", url);
    }

    Ok((repo_path, skills))
}

/// Delete a source: remove all its central symlinks (skills + agents) and delete the source directory.
pub fn delete_source(
    group: &SourceGroup,
    skills_dir: &Path,
    agents_dir: &Path,
    commands_dir: &Path,
) -> anyhow::Result<()> {
    // Remove all central symlinks for this source's skills
    for skill in &group.skills {
        if skill.install_status == SkillInstallStatus::Installed {
            uninstall_skill(&skill.name, skills_dir)?;
        }
    }

    // Remove all central symlinks for this source's agents
    for agent in &group.agents {
        if agent.install_status == SkillInstallStatus::Installed {
            uninstall_agent(&agent.name, agents_dir)?;
        }
    }

    // Remove all central symlinks for this source's commands
    for command in &group.commands {
        if command.install_status == SkillInstallStatus::Installed {
            uninstall_command(&command.name, commands_dir)?;
        }
    }

    // Delete the source directory
    if group.path.exists() {
        fs::remove_dir_all(&group.path)
            .with_context(|| format!("Failed to delete source: {}", group.path.display()))?;
    }

    Ok(())
}

/// Recursively copy a directory tree. Preserves regular files and subdirectories.
fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Migrate a tool's skills directory to the central store.
/// Moves skills from `skills_link` into `tool_skills_target` (under source_dir/agm_tools/{tool}/),
/// then creates central links pointing to the migrated locations.
pub fn migrate_tool_dir(
    skills_link: &Path,
    tool_skills_target: &Path,
    central_skills: &Path,
    tool_key: &str,
) -> anyhow::Result<usize> {
    let (count, msgs) =
        migrate_tool_dir_quiet(skills_link, tool_skills_target, central_skills, tool_key)?;
    for m in &msgs {
        println!("{}", m);
    }
    Ok(count)
}

/// Like `migrate_tool_dir` but returns messages instead of printing (TUI-safe).
pub fn migrate_tool_dir_quiet(
    skills_link: &Path,
    tool_skills_target: &Path,
    central_skills: &Path,
    tool_key: &str,
) -> anyhow::Result<(usize, Vec<String>)> {
    use anyhow::Context;

    let mut msgs = Vec::new();
    fs::create_dir_all(tool_skills_target)?;
    fs::create_dir_all(central_skills)?;

    let discovered = scan_skills(skills_link);
    let mut migrated = 0;

    for (name, skill_path) in &discovered {
        let effective_name = if !central_skills.join(name).exists() {
            name.clone()
        } else {
            let prefixed = format!("{}_{}", tool_key, name);
            msgs.push(format!(
                "  skill '{}' already in central, renaming to '{}'",
                name, prefixed
            ));
            prefixed
        };

        let dest = tool_skills_target.join(&effective_name);
        let link = central_skills.join(&effective_name);

        if dest.exists() {
            msgs.push(format!("  {} already in store, re-linking", effective_name));
        } else {
            fs::rename(skill_path, &dest)
                .with_context(|| format!("Failed to move skill '{}' to store", effective_name))?;
        }

        if link.symlink_metadata().is_ok() {
            let already_ok = platform::read_dir_link_target(&link)
                .and_then(|t| fs::canonicalize(&t).ok())
                .zip(fs::canonicalize(&dest).ok())
                .map(|(a, b)| a == b)
                .unwrap_or(false);
            if already_ok {
                msgs.push(format!("  {} already linked", effective_name));
                migrated += 1;
                continue;
            }
            platform::remove_link(&link)?;
        }

        platform::link_dir(&dest, &link)
            .with_context(|| format!("Failed to link skill '{}' into central", effective_name))?;

        msgs.push(format!("  {} → {}", effective_name, contract_tilde(&dest)));
        migrated += 1;
    }

    if skills_link.exists() {
        fs::remove_dir_all(skills_link)?;
    }

    Ok((migrated, msgs))
}

/// Migrate a tool's agents directory to the central store (TUI-safe).
/// Moves .md files from `agents_link` into `tool_agents_target` (under
/// source_dir/agm_tools/{tool}/agents/), then creates file links in `central_agents`.
pub fn migrate_agents_dir_quiet(
    agents_link: &Path,
    tool_agents_target: &Path,
    central_agents: &Path,
    tool_key: &str,
) -> anyhow::Result<(usize, Vec<String>)> {
    use anyhow::Context;

    let mut msgs = Vec::new();
    fs::create_dir_all(tool_agents_target)?;
    fs::create_dir_all(central_agents)?;

    let mut migrated = 0;
    let entries: Vec<_> = fs::read_dir(agents_link)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("md")
        })
        .collect();

    for entry in &entries {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy().to_string();
        let src = entry.path();

        let effective_name = if !central_agents.join(&name).exists() {
            name.clone()
        } else {
            let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or(&name);
            let prefixed = format!("{}_{}.md", tool_key, stem);
            msgs.push(format!(
                "  agent '{}' already in central, renaming to '{}'",
                name, prefixed
            ));
            prefixed
        };

        let dest = tool_agents_target.join(&effective_name);
        let link = central_agents.join(&effective_name);

        if dest.exists() {
            msgs.push(format!("  {} already in store, re-linking", effective_name));
        } else {
            fs::rename(&src, &dest)
                .with_context(|| format!("Failed to move agent '{}' to store", effective_name))?;
        }

        if link.symlink_metadata().is_ok() {
            let already_ok = platform::same_file(&link, &dest).unwrap_or(false);
            if already_ok {
                msgs.push(format!("  {} already linked", effective_name));
                migrated += 1;
                continue;
            }
            platform::remove_link(&link)?;
        }

        platform::link_file(&dest, &link)
            .with_context(|| format!("Failed to link agent '{}' into central", effective_name))?;

        msgs.push(format!("  {} → {}", effective_name, contract_tilde(&dest)));
        migrated += 1;
    }

    if agents_link.exists() {
        fs::remove_dir_all(agents_link)?;
    }

    Ok((migrated, msgs))
}

/// Migrate a tool's commands directory to the central store (TUI-safe).
/// Moves .md files from `commands_link` into `tool_commands_target` (under
/// source_dir/agm_tools/{tool}/commands/), then creates file links in `central_commands`.
pub fn migrate_commands_dir_quiet(
    commands_link: &Path,
    tool_commands_target: &Path,
    central_commands: &Path,
    tool_key: &str,
) -> anyhow::Result<(usize, Vec<String>)> {
    use anyhow::Context;

    let mut msgs = Vec::new();
    fs::create_dir_all(tool_commands_target)?;
    fs::create_dir_all(central_commands)?;

    let mut migrated = 0;
    let entries: Vec<_> = fs::read_dir(commands_link)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("md")
        })
        .collect();

    for entry in &entries {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy().to_string();
        let src = entry.path();

        let effective_name = if !central_commands.join(&name).exists() {
            name.clone()
        } else {
            let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or(&name);
            let prefixed = format!("{}_{}.md", tool_key, stem);
            msgs.push(format!(
                "  command '{}' already in central, renaming to '{}'",
                name, prefixed
            ));
            prefixed
        };

        let dest = tool_commands_target.join(&effective_name);
        let link = central_commands.join(&effective_name);

        if dest.exists() {
            msgs.push(format!("  {} already in store, re-linking", effective_name));
        } else {
            fs::rename(&src, &dest)
                .with_context(|| format!("Failed to move command '{}' to store", effective_name))?;
        }

        if link.symlink_metadata().is_ok() {
            let already_ok = platform::same_file(&link, &dest).unwrap_or(false);
            if already_ok {
                msgs.push(format!("  {} already linked", effective_name));
                migrated += 1;
                continue;
            }
            platform::remove_link(&link)?;
        }

        platform::link_file(&dest, &link)
            .with_context(|| format!("Failed to link command '{}' into central", effective_name))?;

        msgs.push(format!("  {} → {}", effective_name, contract_tilde(&dest)));
        migrated += 1;
    }

    if commands_link.exists() {
        fs::remove_dir_all(commands_link)?;
    }

    Ok((migrated, msgs))
}

/// Recursively copy a directory, preserving symlinks.
pub fn copy_dir_all(src: &Path, dst: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let meta = fs::symlink_metadata(&src_path)?;
        if platform::is_dir_link(&src_path) {
            if let Some(target) = platform::read_dir_link_target(&src_path) {
                if dst_path.symlink_metadata().is_ok() {
                    platform::remove_link(&dst_path)?;
                }
                platform::link_dir(&target, &dst_path)?;
            }
        } else if meta.file_type().is_symlink() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linker;
    use tempfile::TempDir;

    #[test]
    fn test_scan_skills_single() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("my-skill");
        fs::create_dir(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# Skill").unwrap();

        let skills = scan_skills(&skill_dir);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].0, "my-skill");
    }

    #[test]
    fn test_scan_skills_multiple() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir(&repo).unwrap();

        let skill1 = repo.join("skills/skill1");
        let skill2 = repo.join("skills/skill2");
        fs::create_dir_all(&skill1).unwrap();
        fs::create_dir_all(&skill2).unwrap();
        fs::write(skill1.join("SKILL.md"), "# Skill1").unwrap();
        fs::write(skill2.join("SKILL.md"), "# Skill2").unwrap();

        let skills = scan_skills(&repo);
        assert_eq!(skills.len(), 2);
    }

    #[test]
    fn test_prune_broken_skills() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir(&skills_dir).unwrap();

        // Create a valid skill and a broken symlink
        let skill_source = tmp.path().join("real-skill");
        fs::create_dir(&skill_source).unwrap();
        fs::write(skill_source.join("SKILL.md"), "# Skill").unwrap();
        platform::link_dir(&skill_source, &skills_dir.join("real-skill")).unwrap();

        let ghost = tmp.path().join("ghost-skill");
        // On Windows, junctions require existing target, so create then delete
        fs::create_dir(&ghost).unwrap();
        platform::link_dir(&ghost, &skills_dir.join("ghost-skill")).unwrap();
        fs::remove_dir(&ghost).unwrap();

        let removed = prune_broken_skills(&skills_dir).unwrap();
        assert_eq!(removed, 1);
        assert!(skills_dir.join("real-skill").exists());
        assert!(!skills_dir.join("ghost-skill").exists());
    }

    #[test]
    fn test_install_skill() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source/my-skill");
        let skills_dir = dir.path().join("skills");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "# My Skill").unwrap();
        fs::create_dir_all(&skills_dir).unwrap();

        install_skill("my-skill", &source, &skills_dir).unwrap();
        let link = skills_dir.join("my-skill");
        assert!(link.exists());
        assert!(platform::is_dir_link(&link));
    }

    #[test]
    fn test_install_skill_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let source_a = dir.path().join("source-a/my-skill");
        let source_b = dir.path().join("source-b/my-skill");
        let skills_dir = dir.path().join("skills");
        fs::create_dir_all(&source_a).unwrap();
        fs::create_dir_all(&source_b).unwrap();
        fs::write(source_a.join("SKILL.md"), "# A").unwrap();
        fs::write(source_b.join("SKILL.md"), "# B").unwrap();
        fs::create_dir_all(&skills_dir).unwrap();

        install_skill("my-skill", &source_a, &skills_dir).unwrap();
        let result = install_skill("my-skill", &source_b, &skills_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_uninstall_skill() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source/my-skill");
        let skills_dir = dir.path().join("skills");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "# My Skill").unwrap();
        fs::create_dir_all(&skills_dir).unwrap();

        install_skill("my-skill", &source, &skills_dir).unwrap();
        assert!(skills_dir.join("my-skill").exists());

        uninstall_skill("my-skill", &skills_dir).unwrap();
        assert!(!skills_dir.join("my-skill").exists());
        assert!(source.exists());
    }

    #[test]
    fn test_uninstall_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let skills_dir = dir.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        let result = uninstall_skill("nonexistent", &skills_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn test_scan_all_sources_empty() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");
        let agents_dir = dir.path().join("agents");
        let commands_dir = dir.path().join("commands");
        let groups = scan_all_sources(&source_dir, &skills_dir, &agents_dir, &commands_dir, &[]);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_scan_all_sources_repo() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");
        let agents_dir = dir.path().join("agents");
        let commands_dir = dir.path().join("commands");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::create_dir_all(&agents_dir).unwrap();

        let repo = source_dir.join("my-repo");
        let skill_a = repo.join("skill-a");
        let skill_b = repo.join("skill-b");
        fs::create_dir_all(&skill_a).unwrap();
        fs::create_dir_all(&skill_b).unwrap();
        fs::write(skill_a.join("SKILL.md"), "# A").unwrap();
        fs::write(skill_b.join("SKILL.md"), "# B").unwrap();

        let url = "https://github.com/user/my-repo.git".to_string();
        let groups = scan_all_sources(&source_dir, &skills_dir, &agents_dir, &commands_dir, &[url.clone()]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "my-repo");
        assert_eq!(groups[0].skills.len(), 2);
        match &groups[0].kind {
            SourceKind::Repo { url: u } => assert_eq!(u.as_deref(), Some(url.as_str())),
            _ => panic!("Expected Repo"),
        }
        assert!(groups[0]
            .skills
            .iter()
            .all(|s| s.install_status == SkillInstallStatus::NotInstalled));
    }

    #[test]
    fn test_scan_all_sources_local() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");
        let agents_dir = dir.path().join("agents");
        let commands_dir = dir.path().join("commands");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::create_dir_all(&agents_dir).unwrap();

        let local = source_dir.join("local").join("my-local");
        let skill = local.join("my-skill");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "# Local").unwrap();

        let groups = scan_all_sources(&source_dir, &skills_dir, &agents_dir, &commands_dir, &[]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "my-local");
        assert!(matches!(groups[0].kind, SourceKind::Local));
    }

    #[test]
    fn test_scan_all_sources_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");
        let agents_dir = dir.path().join("agents");
        let commands_dir = dir.path().join("commands");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::create_dir_all(&agents_dir).unwrap();

        let migrated = source_dir.join("agm_tools").join("claude");
        let skill = migrated.join("my-skill");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "# Migrated").unwrap();

        let groups = scan_all_sources(&source_dir, &skills_dir, &agents_dir, &commands_dir, &[]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "agm_tools/claude");
        match &groups[0].kind {
            SourceKind::Migrated { tool } => assert_eq!(tool, "claude"),
            _ => panic!("Expected Migrated"),
        }
    }

    #[test]
    fn test_scan_all_sources_installed_status() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");
        let agents_dir = dir.path().join("agents");
        let commands_dir = dir.path().join("commands");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::create_dir_all(&agents_dir).unwrap();

        let repo = source_dir.join("my-repo");
        let skill_path = repo.join("cool-skill");
        fs::create_dir_all(&skill_path).unwrap();
        fs::write(skill_path.join("SKILL.md"), "# Cool").unwrap();

        install_skill("cool-skill", &skill_path, &skills_dir).unwrap();

        let groups = scan_all_sources(&source_dir, &skills_dir, &agents_dir, &commands_dir, &[]);
        assert_eq!(
            groups[0].skills[0].install_status,
            SkillInstallStatus::Installed
        );
    }

    #[test]
    fn test_scan_all_sources_conflict_status() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");
        let agents_dir = dir.path().join("agents");
        let commands_dir = dir.path().join("commands");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::create_dir_all(&agents_dir).unwrap();

        let repo_a = source_dir.join("repo-a");
        let skill_a = repo_a.join("common-skill");
        fs::create_dir_all(&skill_a).unwrap();
        fs::write(skill_a.join("SKILL.md"), "# A").unwrap();

        let repo_b = source_dir.join("repo-b");
        let skill_b = repo_b.join("common-skill");
        fs::create_dir_all(&skill_b).unwrap();
        fs::write(skill_b.join("SKILL.md"), "# B").unwrap();

        install_skill("common-skill", &skill_a, &skills_dir).unwrap();

        let groups = scan_all_sources(&source_dir, &skills_dir, &agents_dir, &commands_dir, &[]);
        let group_a = groups.iter().find(|g| g.name == "repo-a").unwrap();
        let group_b = groups.iter().find(|g| g.name == "repo-b").unwrap();
        assert_eq!(
            group_a.skills[0].install_status,
            SkillInstallStatus::Installed
        );
        assert_eq!(
            group_b.skills[0].install_status,
            SkillInstallStatus::Conflict
        );
    }

    #[test]
    fn test_add_local_copy() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original");
        let skill = original.join("my-skill");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "# Test").unwrap();

        let source_dir = dir.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();

        let (dest, skills) = add_local_copy(&original, &source_dir).unwrap();

        // Copied to source_dir/local/{name}
        assert!(dest.starts_with(source_dir.join("local")));
        assert!(dest.exists());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].0, "my-skill");
        // Original still exists
        assert!(original.exists());
    }

    #[test]
    fn test_add_local_copy_no_skills() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty");
        fs::create_dir_all(&empty).unwrap();
        let source_dir = dir.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();

        let result = add_local_copy(&empty, &source_dir);
        assert!(result.is_err());
        // Should not have created any directory
        assert!(!source_dir.join("local").exists());
    }

    #[test]
    fn test_add_local_copy_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original");
        let skill = original.join("my-skill");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "# Test").unwrap();

        let source_dir = dir.path().join("source");
        let existing = source_dir.join("local").join("original");
        fs::create_dir_all(&existing).unwrap();

        let result = add_local_copy(&original, &source_dir);
        assert!(result.is_err()); // Already exists
    }

    #[test]
    fn test_delete_source() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");

        // Create a source with 2 skills
        let repo = source_dir.join("my-repo");
        let skill_a = repo.join("skill-a");
        let skill_b = repo.join("skill-b");
        fs::create_dir_all(&skill_a).unwrap();
        fs::create_dir_all(&skill_b).unwrap();
        fs::write(skill_a.join("SKILL.md"), "# A").unwrap();
        fs::write(skill_b.join("SKILL.md"), "# B").unwrap();
        fs::create_dir_all(&skills_dir).unwrap();

        // Install both
        install_skill("skill-a", &skill_a, &skills_dir).unwrap();
        install_skill("skill-b", &skill_b, &skills_dir).unwrap();

        let group = SourceGroup {
            name: "my-repo".to_string(),
            kind: SourceKind::Repo { url: None },
            path: repo.clone(),
            skills: vec![
                SkillInfo {
                    name: "skill-a".to_string(),
                    source_path: skill_a,
                    install_status: SkillInstallStatus::Installed,
                },
                SkillInfo {
                    name: "skill-b".to_string(),
                    source_path: skill_b,
                    install_status: SkillInstallStatus::Installed,
                },
            ],
            agents: vec![],
            commands: vec![],
        };

        delete_source(&group, &skills_dir, &dir.path().join("agents"), &dir.path().join("commands")).unwrap();

        // Central links removed
        assert!(!skills_dir.join("skill-a").exists());
        assert!(!skills_dir.join("skill-b").exists());
        // Source directory removed
        assert!(!repo.exists());
    }

    #[test]
    fn test_scan_agents() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("my-repo");
        let agents = repo.join("agents");
        fs::create_dir_all(&agents).unwrap();
        fs::write(agents.join("helper.md"), "# Helper").unwrap();
        fs::write(agents.join("reviewer.md"), "# Reviewer").unwrap();
        fs::write(agents.join("README.txt"), "not an agent").unwrap();

        let found = scan_agents(&repo);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, "helper");
        assert_eq!(found[1].0, "reviewer");
    }

    #[test]
    fn test_scan_agents_no_dir() {
        let tmp = TempDir::new().unwrap();
        let found = scan_agents(tmp.path());
        assert!(found.is_empty());
    }

    #[test]
    fn test_install_agent() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("source/agents/helper.md");
        let agents_dir = dir.path().join("agents");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "# Helper Agent").unwrap();
        fs::create_dir_all(&agents_dir).unwrap();

        install_agent("helper", &source, &agents_dir).unwrap();
        let link = agents_dir.join("helper.md");
        assert!(link.exists());
        assert_eq!(fs::read_to_string(&link).unwrap(), "# Helper Agent");
    }

    #[test]
    fn test_install_agent_conflict() {
        let dir = TempDir::new().unwrap();
        let source_a = dir.path().join("a/agents/helper.md");
        let source_b = dir.path().join("b/agents/helper.md");
        let agents_dir = dir.path().join("agents");
        fs::create_dir_all(source_a.parent().unwrap()).unwrap();
        fs::create_dir_all(source_b.parent().unwrap()).unwrap();
        fs::write(&source_a, "# A").unwrap();
        fs::write(&source_b, "# B").unwrap();
        fs::create_dir_all(&agents_dir).unwrap();

        install_agent("helper", &source_a, &agents_dir).unwrap();
        let result = install_agent("helper", &source_b, &agents_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_uninstall_agent() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("source/agents/helper.md");
        let agents_dir = dir.path().join("agents");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "# Helper").unwrap();
        fs::create_dir_all(&agents_dir).unwrap();

        install_agent("helper", &source, &agents_dir).unwrap();
        assert!(agents_dir.join("helper.md").exists());

        uninstall_agent("helper", &agents_dir).unwrap();
        assert!(!agents_dir.join("helper.md").exists());
        assert!(source.exists());
    }

    // On Windows, link_file uses hard links which cannot become "broken"
    // when the source is deleted — the data persists through the hard link.
    #[cfg(unix)]
    #[test]
    fn test_prune_broken_agents() {
        let tmp = TempDir::new().unwrap();
        let agents_dir = tmp.path().join("agents");
        fs::create_dir(&agents_dir).unwrap();

        // Create a valid agent link
        let real_source = tmp.path().join("real.md");
        fs::write(&real_source, "# Real").unwrap();
        install_agent("real", &real_source, &agents_dir).unwrap();

        // Create a broken agent link
        let ghost = tmp.path().join("ghost.md");
        fs::write(&ghost, "# Ghost").unwrap();
        install_agent("ghost", &ghost, &agents_dir).unwrap();
        fs::remove_file(&ghost).unwrap();

        let removed = prune_broken_agents(&agents_dir).unwrap();
        assert_eq!(removed, 1);
        assert!(agents_dir.join("real.md").exists());
        assert!(!agents_dir.join("ghost.md").exists());
    }

    #[test]
    fn test_scan_all_sources_with_agents() {
        let dir = TempDir::new().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");
        let agents_dir = dir.path().join("agents");
        let commands_dir = dir.path().join("commands");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::create_dir_all(&agents_dir).unwrap();

        let repo = source_dir.join("my-repo");
        let skill = repo.join("my-skill");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "# Skill").unwrap();

        let agent_dir = repo.join("agents");
        fs::create_dir_all(&agent_dir).unwrap();
        fs::write(agent_dir.join("helper.md"), "# Helper").unwrap();

        let groups = scan_all_sources(&source_dir, &skills_dir, &agents_dir, &commands_dir, &[]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].skills.len(), 1);
        assert_eq!(groups[0].agents.len(), 1);
        assert_eq!(groups[0].agents[0].name, "helper");
    }

    #[test]
    fn test_update_all_with_progress_empty_source() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        let agents_dir = tmp.path().join("agents");
        let commands_dir = tmp.path().join("commands");
        let source_dir = tmp.path().join("source");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::create_dir_all(&agents_dir).unwrap();
        fs::create_dir_all(&source_dir).unwrap();

        let mut events = Vec::new();
        update_all_with_progress(&skills_dir, &agents_dir, &commands_dir, &source_dir, |e| {
            events.push(e);
        });

        assert_eq!(events.len(), 1);
        match &events[0] {
            UpdateProgress::AllDone {
                total,
                updated,
                new_skills,
                new_agents,
                new_commands,
            } => {
                assert_eq!(*total, 0);
                assert_eq!(*updated, 0);
                assert_eq!(*new_skills, 0);
                assert_eq!(*new_agents, 0);
                assert_eq!(*new_commands, 0);
            }
            other => panic!("Expected AllDone, got {:?}", other),
        }
    }

    #[test]
    fn test_update_all_with_progress_nonexistent_source() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        let agents_dir = tmp.path().join("agents");
        let commands_dir = tmp.path().join("commands");
        let source_dir = tmp.path().join("nonexistent");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::create_dir_all(&agents_dir).unwrap();
        // source_dir intentionally not created

        let mut events = Vec::new();
        update_all_with_progress(&skills_dir, &agents_dir, &commands_dir, &source_dir, |e| {
            events.push(e);
        });

        assert_eq!(events.len(), 1);
        match &events[0] {
            UpdateProgress::AllDone { total, .. } => assert_eq!(*total, 0),
            other => panic!("Expected AllDone, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------
    // migrate_tool_dir_quiet tests
    // ------------------------------------------------------------------

    #[test]
    fn test_migrate_tool_dir_quiet_basic() {
        let tmp = TempDir::new().unwrap();
        // Tool's local skills dir with 2 skills
        let skills_dir = tmp.path().join("tool_skills");
        let s1 = skills_dir.join("alpha");
        let s2 = skills_dir.join("beta");
        fs::create_dir_all(&s1).unwrap();
        fs::create_dir_all(&s2).unwrap();
        fs::write(s1.join("SKILL.md"), "# alpha").unwrap();
        fs::write(s2.join("SKILL.md"), "# beta").unwrap();

        let store = tmp.path().join("store");
        let central = tmp.path().join("central");

        let (count, msgs) = migrate_tool_dir_quiet(&skills_dir, &store, &central, "test").unwrap();

        assert_eq!(count, 2);
        assert!(!msgs.is_empty());
        // Original dir removed
        assert!(!skills_dir.exists());
        // Items in store
        assert!(store.join("alpha").join("SKILL.md").exists());
        assert!(store.join("beta").join("SKILL.md").exists());
        // Links in central
        assert!(platform::is_dir_link(&central.join("alpha")));
        assert!(platform::is_dir_link(&central.join("beta")));
    }

    #[test]
    fn test_migrate_tool_dir_quiet_conflict_rename() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("tool_skills");
        let s1 = skills_dir.join("shared");
        fs::create_dir_all(&s1).unwrap();
        fs::write(s1.join("SKILL.md"), "# shared").unwrap();

        let store = tmp.path().join("store");
        let central = tmp.path().join("central");
        // Pre-existing skill with same name in central
        fs::create_dir_all(central.join("shared")).unwrap();

        let (count, msgs) =
            migrate_tool_dir_quiet(&skills_dir, &store, &central, "mytool").unwrap();

        assert_eq!(count, 1);
        // Should be renamed with tool prefix
        assert!(store.join("mytool_shared").exists());
        assert!(central.join("mytool_shared").exists());
        // Messages mention the rename
        assert!(msgs.iter().any(|m| m.contains("renaming")));
    }

    #[test]
    fn test_migrate_tool_dir_quiet_empty() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("empty_skills");
        fs::create_dir_all(&skills_dir).unwrap();
        // No skills inside

        let store = tmp.path().join("store");
        let central = tmp.path().join("central");

        let (count, _msgs) = migrate_tool_dir_quiet(&skills_dir, &store, &central, "test").unwrap();
        assert_eq!(count, 0);
        // Original dir still removed
        assert!(!skills_dir.exists());
    }

    // ------------------------------------------------------------------
    // migrate_agents_dir_quiet tests
    // ------------------------------------------------------------------

    #[test]
    fn test_migrate_agents_dir_quiet_basic() {
        let tmp = TempDir::new().unwrap();
        let agents_dir = tmp.path().join("tool_agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("helper.md"), "# helper agent").unwrap();
        fs::write(agents_dir.join("reviewer.md"), "# reviewer agent").unwrap();
        fs::write(agents_dir.join("README"), "not an agent").unwrap(); // non-.md file

        let store = tmp.path().join("store_agents");
        let central = tmp.path().join("central_agents");

        let (count, msgs) =
            migrate_agents_dir_quiet(&agents_dir, &store, &central, "test").unwrap();

        assert_eq!(count, 2);
        assert!(!msgs.is_empty());
        // Original dir removed
        assert!(!agents_dir.exists());
        // Items in store
        assert!(store.join("helper.md").exists());
        assert!(store.join("reviewer.md").exists());
        // Non-md file not migrated (lost with dir removal — intentional)
        // Links in central (file links)
        assert!(platform::same_file(&central.join("helper.md"), &store.join("helper.md")).unwrap());
        assert!(platform::same_file(&central.join("reviewer.md"), &store.join("reviewer.md")).unwrap());
    }

    #[test]
    fn test_migrate_agents_dir_quiet_conflict_rename() {
        let tmp = TempDir::new().unwrap();
        let agents_dir = tmp.path().join("tool_agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("shared.md"), "# shared agent").unwrap();

        let store = tmp.path().join("store_agents");
        let central = tmp.path().join("central_agents");
        // Pre-existing agent with same name in central
        fs::create_dir_all(&central).unwrap();
        fs::write(central.join("shared.md"), "# existing agent").unwrap();

        let (count, msgs) =
            migrate_agents_dir_quiet(&agents_dir, &store, &central, "mytool").unwrap();

        assert_eq!(count, 1);
        assert!(store.join("mytool_shared.md").exists());
        assert!(platform::same_file(&central.join("mytool_shared.md"), &store.join("mytool_shared.md")).unwrap());
        // Original preserved
        assert_eq!(
            fs::read_to_string(central.join("shared.md")).unwrap(),
            "# existing agent"
        );
        assert!(msgs.iter().any(|m| m.contains("renaming")));
    }

    #[test]
    fn test_migrate_agents_dir_quiet_empty() {
        let tmp = TempDir::new().unwrap();
        let agents_dir = tmp.path().join("empty_agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let store = tmp.path().join("store_agents");
        let central = tmp.path().join("central_agents");

        let (count, _msgs) =
            migrate_agents_dir_quiet(&agents_dir, &store, &central, "test").unwrap();
        assert_eq!(count, 0);
        assert!(!agents_dir.exists());
    }

    // ------------------------------------------------------------------
    // Full cycle: migrate → link → unlink → restore
    // ------------------------------------------------------------------

    #[test]
    fn test_skills_full_cycle_migrate_link_unlink_restore() {
        let tmp = TempDir::new().unwrap();
        // Setup: tool has local skills dir with content
        let tool_skills = tmp.path().join("tool_skills");
        let s1 = tool_skills.join("my-skill");
        fs::create_dir_all(&s1).unwrap();
        fs::write(s1.join("SKILL.md"), "# My Skill").unwrap();
        fs::write(s1.join("code.py"), "print('hi')").unwrap();

        let store = tmp.path().join("agm_tools").join("testtool");
        let central = tmp.path().join("central_skills");

        // Step 1: Migrate (simulating handle_blocked_link for skills)
        let (count, _) =
            migrate_tool_dir_quiet(&tool_skills, &store, &central, "testtool").unwrap();
        assert_eq!(count, 1);
        assert!(!tool_skills.exists());
        assert!(store.join("my-skill").join("SKILL.md").exists());
        assert!(store.join("my-skill").join("code.py").exists());

        // Step 2: Create link
        linker::create_link_quiet(&tool_skills, &central, "skills", true).unwrap();
        assert_eq!(
            linker::check_link(&tool_skills, &central, true),
            linker::LinkStatus::Linked
        );

        // Step 3: Unlink
        linker::remove_link_quiet(&tool_skills, "skills", true).unwrap();
        assert!(!tool_skills.exists());

        // Step 4: Restore (simulating recover_after_unlink for Skills)
        fs::create_dir_all(&tool_skills).unwrap();
        let mut restored = 0;
        for entry in fs::read_dir(&store).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            if !path.is_dir() || !path.join("SKILL.md").exists() {
                continue;
            }
            let dest = tool_skills.join(&name);
            if !dest.exists() {
                fs::rename(&path, &dest).unwrap();
                restored += 1;
            }
        }
        assert_eq!(restored, 1);
        assert!(tool_skills.join("my-skill").join("SKILL.md").exists());
        assert!(tool_skills.join("my-skill").join("code.py").exists());
    }

    #[test]
    fn test_agents_full_cycle_migrate_link_unlink_restore() {
        let tmp = TempDir::new().unwrap();
        // Setup: tool has local agents dir with .md files
        let tool_agents = tmp.path().join("tool_agents");
        fs::create_dir_all(&tool_agents).unwrap();
        fs::write(tool_agents.join("coder.md"), "# Coder Agent").unwrap();
        fs::write(tool_agents.join("tester.md"), "# Tester Agent").unwrap();

        let store_agents = tmp.path().join("agm_tools").join("testtool").join("agents");
        let central = tmp.path().join("central_agents");

        // Step 1: Migrate
        let (count, _) =
            migrate_agents_dir_quiet(&tool_agents, &store_agents, &central, "testtool").unwrap();
        assert_eq!(count, 2);
        assert!(!tool_agents.exists());
        assert!(store_agents.join("coder.md").exists());
        assert!(store_agents.join("tester.md").exists());

        // Step 2: Create link
        linker::create_link_quiet(&tool_agents, &central, "agents", true).unwrap();
        assert_eq!(
            linker::check_link(&tool_agents, &central, true),
            linker::LinkStatus::Linked
        );

        // Step 3: Unlink
        linker::remove_link_quiet(&tool_agents, "agents", true).unwrap();
        assert!(!tool_agents.exists());

        // Step 4: Restore (simulating recover_after_unlink for Agents)
        assert!(store_agents.exists());
        fs::rename(&store_agents, &tool_agents).unwrap();
        assert!(tool_agents.join("coder.md").exists());
        assert!(tool_agents.join("tester.md").exists());
        assert_eq!(
            fs::read_to_string(tool_agents.join("coder.md")).unwrap(),
            "# Coder Agent"
        );
    }

    #[test]
    fn test_prompt_full_cycle_backup_link_unlink_restore() {
        let tmp = TempDir::new().unwrap();
        // Setup: tool has a local prompt file
        let prompt_path = tmp.path().join("AGENTS.md");
        fs::write(&prompt_path, "# My Custom Prompt").unwrap();

        let central_prompt = tmp.path().join("central").join("MASTER.md");
        fs::create_dir_all(central_prompt.parent().unwrap()).unwrap();
        fs::write(&central_prompt, "# Central Prompt").unwrap();

        // Step 1: Backup (with_extension replaces .md, so AGENTS.md → AGENTS.{ts}.bak)
        let backup = prompt_path.with_extension("20990101_120000.bak");
        fs::rename(&prompt_path, &backup).unwrap();
        assert!(!prompt_path.exists());
        assert!(backup.exists());

        // Step 2: Create link
        linker::create_link_quiet(&prompt_path, &central_prompt, "prompt", false).unwrap();
        assert_eq!(
            linker::check_link(&prompt_path, &central_prompt, false),
            linker::LinkStatus::Linked
        );
        assert_eq!(
            fs::read_to_string(&prompt_path).unwrap(),
            "# Central Prompt"
        );

        // Step 3: Unlink
        linker::remove_link_quiet(&prompt_path, "prompt", false).unwrap();
        assert!(!prompt_path.exists());

        // Step 4: Restore (simulating recover_after_unlink for Prompt)
        // Use file_stem to match backup pattern (same logic as tool.rs)
        let stem = "AGENTS";
        let prefix = format!("{}.", stem);
        let parent = tmp.path();
        let mut backups: Vec<_> = fs::read_dir(parent)
            .unwrap()
            .flatten()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.starts_with(&prefix) && n.ends_with(".bak")
            })
            .collect();
        backups.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
        assert!(!backups.is_empty());
        let latest = backups.first().unwrap();
        fs::rename(latest.path(), &prompt_path).unwrap();

        // Verify original content restored
        assert!(prompt_path.exists());
        assert_eq!(
            fs::read_to_string(&prompt_path).unwrap(),
            "# My Custom Prompt"
        );
    }

    // ------------------------------------------------------------------
    // Commands tests
    // ------------------------------------------------------------------

    #[test]
    fn test_scan_commands() {
        let tmp = tempfile::tempdir().unwrap();
        let commands_dir = tmp.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(commands_dir.join("fix.md"), "# Fix command").unwrap();
        fs::write(commands_dir.join("review.md"), "# Review command").unwrap();
        fs::write(commands_dir.join("not-a-command.txt"), "ignored").unwrap();

        let result = scan_commands(tmp.path());
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "fix");
        assert_eq!(result[1].0, "review");
    }

    #[test]
    fn test_scan_commands_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let result = scan_commands(tmp.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_install_uninstall_command() {
        let tmp = tempfile::tempdir().unwrap();
        let source_dir = tmp.path().join("source");
        let central_dir = tmp.path().join("central");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&central_dir).unwrap();

        let cmd_file = source_dir.join("deploy.md");
        fs::write(&cmd_file, "# Deploy command").unwrap();

        // Install
        install_command("deploy", &cmd_file, &central_dir).unwrap();
        let link = central_dir.join("deploy.md");
        assert!(link.symlink_metadata().is_ok());
        assert!(link.exists());

        // Install again (idempotent)
        install_command("deploy", &cmd_file, &central_dir).unwrap();

        // Uninstall
        uninstall_command("deploy", &central_dir).unwrap();
        assert!(central_dir.join("deploy.md").symlink_metadata().is_err());

        // Uninstall again (idempotent)
        uninstall_command("deploy", &central_dir).unwrap();
    }

    #[test]
    fn test_install_command_conflict() {
        let tmp = tempfile::tempdir().unwrap();
        let source1 = tmp.path().join("source1");
        let source2 = tmp.path().join("source2");
        let central = tmp.path().join("central");
        fs::create_dir_all(&source1).unwrap();
        fs::create_dir_all(&source2).unwrap();
        fs::create_dir_all(&central).unwrap();

        let cmd1 = source1.join("test.md");
        let cmd2 = source2.join("test.md");
        fs::write(&cmd1, "# Command from source 1").unwrap();
        fs::write(&cmd2, "# Command from source 2").unwrap();

        install_command("test", &cmd1, &central).unwrap();
        let result = install_command("test", &cmd2, &central);
        assert!(result.is_err());
    }

    #[test]
    fn test_prune_broken_commands() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let central = tmp.path().join("central");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&central).unwrap();

        let cmd_file = source.join("ephemeral.md");
        fs::write(&cmd_file, "# Ephemeral").unwrap();

        install_command("ephemeral", &cmd_file, &central).unwrap();
        assert!(central.join("ephemeral.md").exists());

        // Remove source — link is now broken
        fs::remove_file(&cmd_file).unwrap();
        assert!(!central.join("ephemeral.md").exists()); // target gone

        let removed = prune_broken_commands(&central).unwrap();
        assert_eq!(removed, 1);
        assert!(central.join("ephemeral.md").symlink_metadata().is_err());
    }

    #[test]
    fn test_migrate_commands_dir_quiet() {
        let tmp = tempfile::tempdir().unwrap();
        let tool_commands = tmp.path().join("tool_commands");
        let store = tmp.path().join("store");
        let central = tmp.path().join("central");
        fs::create_dir_all(&tool_commands).unwrap();

        fs::write(tool_commands.join("build.md"), "# Build").unwrap();
        fs::write(tool_commands.join("test.md"), "# Test").unwrap();

        let (count, msgs) =
            migrate_commands_dir_quiet(&tool_commands, &store, &central, "claude").unwrap();
        assert_eq!(count, 2);
        assert!(!msgs.is_empty());

        // Original dir should be removed
        assert!(!tool_commands.exists());

        // Store should have files
        assert!(store.join("build.md").exists());
        assert!(store.join("test.md").exists());

        // Central should have symlinks
        assert!(central.join("build.md").symlink_metadata().is_ok());
        assert!(central.join("test.md").symlink_metadata().is_ok());
    }
}
