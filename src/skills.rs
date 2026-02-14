use anyhow::Context;
use colored::Colorize;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

use crate::paths::contract_tilde;

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
        if path.is_symlink() || path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                // Resolve symlink to get source path
                let source = if path.is_symlink() {
                    fs::read_link(&path).unwrap_or(path.clone())
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

        unix_fs::symlink(&skill_path, &link_path)
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

    if skill_path.is_symlink() {
        fs::remove_file(&skill_path)?;
        println!("{} Removed skill: {}", " ok ".green(), name);
    } else if skill_path.is_dir() {
        // It's a real directory, not a symlink - be cautious
        anyhow::bail!(
            "'{}' is a directory, not a symlink. Remove manually if needed.",
            name
        );
    } else {
        anyhow::bail!("'{}' is not a symlink", name);
    }

    Ok(())
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

/// Git pull all skill source repos (deduplicating by git root)
pub fn update_all(skills_dir: &Path) -> anyhow::Result<()> {
    if !skills_dir.is_dir() {
        anyhow::bail!("Skills directory does not exist: {}", skills_dir.display());
    }

    let mut git_roots = std::collections::HashSet::new();

    // Iterate all symlinks in skills directory
    for entry in fs::read_dir(skills_dir)? {
        let entry = entry?;
        let link_path = entry.path();

        if link_path.is_symlink() {
            // Resolve symlink to real path
            if let Ok(target) = fs::read_link(&link_path) {
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

    for git_root in git_roots {
        println!("Updating {}...", contract_tilde(&git_root));
        let status = std::process::Command::new("git")
            .args(["pull"])
            .current_dir(&git_root)
            .status()?;

        if status.success() {
            println!("{} Updated {}\n", " ok ".green(), contract_tilde(&git_root));
        } else {
            println!(
                "{} Failed to update {}\n",
                "fail".red(),
                contract_tilde(&git_root)
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
        unix_fs::symlink(&skill_source, &skill_link).unwrap();

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
        assert!(skills_dir.join("my-skill").is_symlink());
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
        unix_fs::symlink(&skill_source, &skill_link).unwrap();

        remove_skill("my-skill", &skills_dir).unwrap();
        assert!(!skill_link.exists());
    }
}
