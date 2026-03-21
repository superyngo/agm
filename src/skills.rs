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
    if link_path.symlink_metadata().is_err() {
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



/// Git pull all skill source repos (deduplicating by git root), then re-sync symlinks
pub fn update_all(skills_dir: &Path, source_dir: &Path) -> anyhow::Result<()> {
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
        println!("  {} Removed {} broken link(s)", "warn".yellow(), pruned);
    }

    // Re-sync: for each repo, find new skills not yet installed and install them
    for git_root in &git_roots {
        let new_skills = scan_skills(git_root);
        let mut added = 0;
        for (name, skill_path) in new_skills {
            let link_path = skills_dir.join(&name);
            if link_path.symlink_metadata().is_err() {
                // Not installed — install it (new skill discovered after pull)
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

/// Try to resolve the git remote URL for a directory.
fn resolve_repo_url(dir_name: &str, path: &Path, skill_repos: &[String]) -> Option<String> {
    for url in skill_repos {
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
                String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
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

/// Scan the source directory and return all sources grouped with their skills and install status.
/// Gracefully returns empty Vec if source_dir doesn't exist.
pub fn scan_all_sources(
    source_dir: &Path,
    skills_dir: &Path,
    skill_repos: &[String],
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
                    groups.push(SourceGroup {
                        name: sub_name,
                        kind: SourceKind::Local,
                        path: sub_path,
                        skills,
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
                    groups.push(SourceGroup {
                        name: format!("agm_tools/{}", tool_name),
                        kind: SourceKind::Migrated { tool: tool_name },
                        path: sub_path,
                        skills,
                    });
                }
            }
        } else {
            let url = resolve_repo_url(&dir_name, &path, skill_repos);
            let skills = scan_skills(&path)
                .into_iter()
                .map(|(name, sp)| SkillInfo {
                    install_status: check_install_status(&name, &sp, skills_dir),
                    name,
                    source_path: sp,
                })
                .collect();
            groups.push(SourceGroup {
                name: dir_name,
                kind: SourceKind::Repo { url },
                path,
                skills,
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
                    String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
                } else {
                    None
                }
            });

        if let Some(ref existing) = existing_url {
            if existing != url {
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

/// Delete a source: remove all its central symlinks and delete the source directory.
pub fn delete_source(group: &SourceGroup, skills_dir: &Path) -> anyhow::Result<()> {
    // Remove all central symlinks for this source's skills
    for skill in &group.skills {
        if skill.install_status == SkillInstallStatus::Installed {
            uninstall_skill(&skill.name, skills_dir)?;
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
        let groups = scan_all_sources(&source_dir, &skills_dir, &[]);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_scan_all_sources_repo() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let repo = source_dir.join("my-repo");
        let skill_a = repo.join("skill-a");
        let skill_b = repo.join("skill-b");
        fs::create_dir_all(&skill_a).unwrap();
        fs::create_dir_all(&skill_b).unwrap();
        fs::write(skill_a.join("SKILL.md"), "# A").unwrap();
        fs::write(skill_b.join("SKILL.md"), "# B").unwrap();

        let url = "https://github.com/user/my-repo.git".to_string();
        let groups = scan_all_sources(&source_dir, &skills_dir, &[url.clone()]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "my-repo");
        assert_eq!(groups[0].skills.len(), 2);
        match &groups[0].kind {
            SourceKind::Repo { url: u } => assert_eq!(u.as_deref(), Some(url.as_str())),
            _ => panic!("Expected Repo"),
        }
        assert!(groups[0].skills.iter().all(|s| s.install_status == SkillInstallStatus::NotInstalled));
    }

    #[test]
    fn test_scan_all_sources_local() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let local = source_dir.join("local").join("my-local");
        let skill = local.join("my-skill");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "# Local").unwrap();

        let groups = scan_all_sources(&source_dir, &skills_dir, &[]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "my-local");
        assert!(matches!(groups[0].kind, SourceKind::Local));
    }

    #[test]
    fn test_scan_all_sources_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let migrated = source_dir.join("agm_tools").join("claude");
        let skill = migrated.join("my-skill");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "# Migrated").unwrap();

        let groups = scan_all_sources(&source_dir, &skills_dir, &[]);
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
        fs::create_dir_all(&skills_dir).unwrap();

        let repo = source_dir.join("my-repo");
        let skill_path = repo.join("cool-skill");
        fs::create_dir_all(&skill_path).unwrap();
        fs::write(skill_path.join("SKILL.md"), "# Cool").unwrap();

        install_skill("cool-skill", &skill_path, &skills_dir).unwrap();

        let groups = scan_all_sources(&source_dir, &skills_dir, &[]);
        assert_eq!(groups[0].skills[0].install_status, SkillInstallStatus::Installed);
    }

    #[test]
    fn test_scan_all_sources_conflict_status() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let skills_dir = dir.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let repo_a = source_dir.join("repo-a");
        let skill_a = repo_a.join("common-skill");
        fs::create_dir_all(&skill_a).unwrap();
        fs::write(skill_a.join("SKILL.md"), "# A").unwrap();

        let repo_b = source_dir.join("repo-b");
        let skill_b = repo_b.join("common-skill");
        fs::create_dir_all(&skill_b).unwrap();
        fs::write(skill_b.join("SKILL.md"), "# B").unwrap();

        install_skill("common-skill", &skill_a, &skills_dir).unwrap();

        let groups = scan_all_sources(&source_dir, &skills_dir, &[]);
        let group_a = groups.iter().find(|g| g.name == "repo-a").unwrap();
        let group_b = groups.iter().find(|g| g.name == "repo-b").unwrap();
        assert_eq!(group_a.skills[0].install_status, SkillInstallStatus::Installed);
        assert_eq!(group_b.skills[0].install_status, SkillInstallStatus::Conflict);
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
        };

        delete_source(&group, &skills_dir).unwrap();

        // Central links removed
        assert!(!skills_dir.join("skill-a").exists());
        assert!(!skills_dir.join("skill-b").exists());
        // Source directory removed
        assert!(!repo.exists());
    }
}
