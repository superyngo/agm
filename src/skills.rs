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

/// A source and all skills it contains
#[derive(Debug, Clone)]
pub struct SourceGroup {
    pub name: String,
    pub kind: SourceKind,
    pub path: PathBuf,
    pub skills: Vec<SkillInfo>,
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

/// List all skills in the central skills directory
pub fn list_skills(skills_dir: &Path) -> anyhow::Result<Vec<(String, PathBuf)>> {
    if !skills_dir.is_dir() {
        return Ok(vec![]);
    }

    let mut skills = Vec::new();
    for entry in fs::read_dir(skills_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() || platform::is_dir_link(&path) {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                // Resolve link to get source path
                let source = if platform::is_dir_link(&path) {
                    platform::read_dir_link_target(&path).unwrap_or_else(|| path.clone())
                } else {
                    path.clone()
                };
                skills.push((name.to_string(), source));
            }
        }
    }

    skills.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(skills)
}

/// Add skill(s) from a local path
pub fn add_local(source: &Path, skills_dir: &Path) -> anyhow::Result<usize> {
    if !source.exists() {
        anyhow::bail!("Source path does not exist: {}", source.display());
    }

    // Ensure skills directory exists
    fs::create_dir_all(skills_dir)?;

    let skills = scan_skills(source);
    if skills.is_empty() {
        anyhow::bail!("No skills found at {}", source.display());
    }

    let mut added = 0;
    for (name, skill_path) in skills {
        let link_path = skills_dir.join(&name);

        if link_path.exists() {
            println!("  {} {} already exists, skipping", "skip".yellow(), name);
            continue;
        }

        platform::link_dir(&skill_path, &link_path)
            .with_context(|| format!("Failed to link skill: {}", name))?;
        println!(
            "  {} {} → {}",
            " ok ".green(),
            name,
            contract_tilde(&skill_path)
        );
        added += 1;
    }

    Ok(added)
}

/// Remove a skill by name
pub fn remove_skill(name: &str, skills_dir: &Path) -> anyhow::Result<()> {
    let skill_path = skills_dir.join(name);

    if !skill_path.exists() {
        anyhow::bail!("Skill '{}' not found", name);
    }

    if platform::is_dir_link(&skill_path) {
        platform::remove_link(&skill_path)?;
        println!("{} Removed skill: {}", " ok ".green(), name);
    } else if skill_path.is_dir() {
        // It's a real directory, not a link — be cautious
        anyhow::bail!(
            "'{}' is a directory, not a link. Remove manually if needed.",
            name
        );
    } else {
        anyhow::bail!("'{}' is not a link", name);
    }

    Ok(())
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
                let source_canon = fs::canonicalize(source_path).unwrap_or(source_path.to_path_buf());
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
    if !link_path.symlink_metadata().is_ok() {
        return Ok(());
    }
    if platform::is_dir_link(&link_path) {
        platform::remove_link(&link_path)?;
    }
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

/// Check if source string is a URL
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

/// Add skills from a git repo URL
pub fn add_from_url(url: &str, source_dir: &Path, skills_dir: &Path) -> anyhow::Result<usize> {
    let name = repo_name_from_url(url);
    let repo_path = source_dir.join(&name);

    if repo_path.is_dir() {
        // git pull
        println!("Updating {} from {}...", name, url);
        let status = std::process::Command::new("git")
            .args(["pull"])
            .current_dir(&repo_path)
            .status()?;
        if !status.success() {
            anyhow::bail!("git pull failed for {}", name);
        }
    } else {
        // git clone
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
        // No skills found, clean up
        std::fs::remove_dir_all(&repo_path)?;
        anyhow::bail!("No skills found in {}. Clone removed.", url);
    }

    add_local(&repo_path, skills_dir)
}

/// Git pull all skill source repos (deduplicating by git root), then re-sync symlinks
pub fn update_all(skills_dir: &Path) -> anyhow::Result<()> {
    if !skills_dir.is_dir() {
        anyhow::bail!("Skills directory does not exist: {}", skills_dir.display());
    }

    let mut git_roots = std::collections::HashSet::new();

    // Iterate all symlinks in skills directory
    for entry in fs::read_dir(skills_dir)? {
        let entry = entry?;
        let link_path = entry.path();

        if platform::is_dir_link(&link_path) {
            // Resolve link to real path
            if let Some(target) = platform::read_dir_link_target(&link_path) {
                let real_path = if target.is_absolute() {
                    target
                } else {
                    skills_dir.join(target)
                };

                // Find git root
                if let Ok(git_root) = find_git_root(&real_path) {
                    git_roots.insert(git_root);
                }
            }
        }
    }

    if git_roots.is_empty() {
        println!("No git repositories found in skills directory.");
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

    // Re-sync: prune broken links then add any new skills from all known repos
    println!("{}", "Syncing central skills symlinks...".bold());
    let pruned = prune_broken_skills(skills_dir)?;
    if pruned > 0 {
        println!("  {} Removed {} broken link(s)", "warn".yellow(), pruned);
    }
    for git_root in &git_roots {
        let added = add_local(git_root, skills_dir)?;
        if added > 0 {
            println!(
                "  {} {} new skill(s) from {}",
                " ok ".green(),
                added,
                contract_tilde(git_root)
            );
        }
    }

    Ok(())
}

/// Find git root for a path
fn find_git_root(path: &Path) -> anyhow::Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path.parent().unwrap_or(path))
        .output()?;

    if output.status.success() {
        let root = String::from_utf8(output.stdout)?.trim().to_string();
        Ok(PathBuf::from(root))
    } else {
        anyhow::bail!("Not a git repository")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_list_skills() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir(&skills_dir).unwrap();

        let skill_source = tmp.path().join("source/skill1");
        fs::create_dir_all(&skill_source).unwrap();
        fs::write(skill_source.join("SKILL.md"), "# Skill").unwrap();

        let skill_link = skills_dir.join("skill1");
        platform::link_dir(&skill_source, &skill_link).unwrap();

        let skills = list_skills(&skills_dir).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].0, "skill1");
    }

    #[test]
    fn test_add_local_single() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        let skill_source = tmp.path().join("my-skill");
        fs::create_dir(&skill_source).unwrap();
        fs::write(skill_source.join("SKILL.md"), "# Skill").unwrap();

        let added = add_local(&skill_source, &skills_dir).unwrap();
        assert_eq!(added, 1);
        assert!(skills_dir.join("my-skill").exists());
    }

    #[test]
    fn test_add_local_idempotent() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        let skill_source = tmp.path().join("my-skill");
        fs::create_dir(&skill_source).unwrap();
        fs::write(skill_source.join("SKILL.md"), "# Skill").unwrap();

        add_local(&skill_source, &skills_dir).unwrap();
        let added = add_local(&skill_source, &skills_dir).unwrap();
        assert_eq!(added, 0); // skipped
    }

    #[test]
    fn test_remove_skill() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir(&skills_dir).unwrap();

        let skill_source = tmp.path().join("my-skill");
        fs::create_dir(&skill_source).unwrap();
        fs::write(skill_source.join("SKILL.md"), "# Skill").unwrap();

        let skill_link = skills_dir.join("my-skill");
        platform::link_dir(&skill_source, &skill_link).unwrap();

        remove_skill("my-skill", &skills_dir).unwrap();
        assert!(!skill_link.exists());
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
}
