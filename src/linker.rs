use anyhow::Context;
use colored::Colorize;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;

/// Status of a symlink check
#[derive(Debug, PartialEq)]
pub enum LinkStatus {
    /// Correct symlink pointing to expected target
    Linked,
    /// Symlink exists but points to wrong target
    Wrong(String),
    /// Path exists but is not a symlink (regular file/dir)
    Blocked,
    /// Nothing exists at the path
    Missing,
    /// Symlink exists but target doesn't exist
    Broken,
}

/// Check the status of a symlink at `link_path` that should point to `expected_target`
pub fn check_link(link_path: &Path, expected_target: &Path) -> LinkStatus {
    if link_path.symlink_metadata().is_err() {
        return LinkStatus::Missing;
    }

    match fs::read_link(link_path) {
        Ok(actual_target) => {
            // Canonicalize both for comparison
            let actual = fs::canonicalize(&actual_target)
                .or_else(|_| fs::canonicalize(link_path.parent().unwrap().join(&actual_target)))
                .unwrap_or(actual_target.clone());
            let expected =
                fs::canonicalize(expected_target).unwrap_or_else(|_| expected_target.to_path_buf());

            if actual == expected {
                // Check if target actually exists
                if expected.exists() {
                    LinkStatus::Linked
                } else {
                    LinkStatus::Broken
                }
            } else {
                LinkStatus::Wrong(actual.display().to_string())
            }
        }
        Err(_) => LinkStatus::Blocked,
    }
}

/// Create a symlink from `link_path` to `target`. Returns Ok(true) if created, Ok(false) if skipped.
pub fn create_link(link_path: &Path, target: &Path, label: &str) -> anyhow::Result<bool> {
    match check_link(link_path, target) {
        LinkStatus::Linked => {
            println!("  {} {} already linked", "skip".yellow(), label);
            Ok(false)
        }
        LinkStatus::Missing => {
            unix_fs::symlink(target, link_path).with_context(|| {
                format!(
                    "Failed to create symlink: {} → {}",
                    link_path.display(),
                    target.display()
                )
            })?;
            println!("  {} {} → {}", " ok ".green(), label, target.display());
            Ok(true)
        }
        LinkStatus::Broken => {
            // Remove broken symlink and recreate
            fs::remove_file(link_path)?;
            unix_fs::symlink(target, link_path)?;
            println!("  {} {} (repaired broken link)", " ok ".green(), label);
            Ok(true)
        }
        LinkStatus::Wrong(actual) => {
            println!(
                "  {} {} points to {} (expected {})",
                "warn".red(),
                label,
                actual,
                target.display()
            );
            Ok(false)
        }
        LinkStatus::Blocked => {
            println!(
                "  {} {} exists but is not a symlink, skipping",
                "warn".red(),
                label
            );
            Ok(false)
        }
    }
}

/// Remove a symlink if it exists and points to expected target
pub fn remove_link(link_path: &Path, label: &str) -> anyhow::Result<bool> {
    if link_path.symlink_metadata().is_ok() {
        if fs::read_link(link_path).is_ok() {
            fs::remove_file(link_path)?;
            println!("  {} {} removed", " ok ".green(), label);
            Ok(true)
        } else {
            println!("  {} {} is not a symlink, skipping", "warn".red(), label);
            Ok(false)
        }
    } else {
        println!("  {} {} not found", "skip".yellow(), label);
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_check_link_missing() {
        let tmp = TempDir::new().unwrap();
        let link = tmp.path().join("link");
        let target = tmp.path().join("target");
        assert_eq!(check_link(&link, &target), LinkStatus::Missing);
    }

    #[test]
    fn test_check_link_correct() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link");
        unix_fs::symlink(&target, &link).unwrap();
        assert_eq!(check_link(&link, &target), LinkStatus::Linked);
    }

    #[test]
    fn test_check_link_wrong() {
        let tmp = TempDir::new().unwrap();
        let target1 = tmp.path().join("target1");
        let target2 = tmp.path().join("target2");
        fs::write(&target1, "1").unwrap();
        fs::write(&target2, "2").unwrap();
        let link = tmp.path().join("link");
        unix_fs::symlink(&target1, &link).unwrap();
        assert!(matches!(check_link(&link, &target2), LinkStatus::Wrong(_)));
    }

    #[test]
    fn test_check_link_blocked() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("file");
        fs::write(&path, "not a symlink").unwrap();
        let target = tmp.path().join("target");
        assert_eq!(check_link(&path, &target), LinkStatus::Blocked);
    }

    #[test]
    fn test_create_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link");
        let created = create_link(&link, &target, "test").unwrap();
        assert!(created);
        assert_eq!(fs::read_link(&link).unwrap(), target);
    }

    #[test]
    fn test_create_link_idempotent() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link");
        create_link(&link, &target, "test").unwrap();
        let created = create_link(&link, &target, "test").unwrap();
        assert!(!created); // skipped
    }

    #[test]
    fn test_remove_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link");
        unix_fs::symlink(&target, &link).unwrap();
        let removed = remove_link(&link, "test").unwrap();
        assert!(removed);
        assert!(!link.exists());
    }
}
