use anyhow::Context;
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::platform;

/// Status of a link check
#[derive(Debug, PartialEq)]
pub enum LinkStatus {
    /// Correct link pointing to expected target
    Linked,
    /// Link exists but points to wrong target
    Wrong(String),
    /// Path exists but is not a link (regular file/dir)
    Blocked,
    /// Nothing exists at the path
    Missing,
    /// Link exists but target doesn't exist
    Broken,
}

/// Check the status of a link at `link_path` that should point to `expected_target`.
/// `is_dir` selects directory-link detection (symlink/junction) vs file-link detection
/// (symlink/hardlink).
pub fn check_link(link_path: &Path, expected_target: &Path, is_dir: bool) -> LinkStatus {
    if link_path.symlink_metadata().is_err() {
        return LinkStatus::Missing;
    }

    if is_dir {
        check_dir_link(link_path, expected_target)
    } else {
        check_file_link(link_path, expected_target)
    }
}

/// Check a directory link (symlink on Unix, junction on Windows).
fn check_dir_link(link_path: &Path, expected_target: &Path) -> LinkStatus {
    match platform::read_dir_link_target(link_path) {
        Some(actual_target) => {
            let actual = fs::canonicalize(&actual_target)
                .or_else(|_| fs::canonicalize(link_path.parent().unwrap().join(&actual_target)))
                .unwrap_or(actual_target);
            let expected =
                fs::canonicalize(expected_target).unwrap_or_else(|_| expected_target.to_path_buf());

            if actual == expected {
                if expected.exists() {
                    LinkStatus::Linked
                } else {
                    LinkStatus::Broken
                }
            } else {
                LinkStatus::Wrong(actual.display().to_string())
            }
        }
        None => LinkStatus::Blocked,
    }
}

/// Check a file link (symlink on Unix, hardlink on Windows).
fn check_file_link(link_path: &Path, expected_target: &Path) -> LinkStatus {
    // Try read_link first (works for Unix symlinks)
    if let Ok(actual_target) = fs::read_link(link_path) {
        let actual = fs::canonicalize(&actual_target)
            .or_else(|_| fs::canonicalize(link_path.parent().unwrap().join(&actual_target)))
            .unwrap_or(actual_target);
        let expected =
            fs::canonicalize(expected_target).unwrap_or_else(|_| expected_target.to_path_buf());

        if actual == expected {
            if expected.exists() {
                LinkStatus::Linked
            } else {
                LinkStatus::Broken
            }
        } else {
            LinkStatus::Wrong(actual.display().to_string())
        }
    } else if expected_target.exists() {
        // read_link failed — could be a hardlink (Windows)
        match platform::same_file(link_path, expected_target) {
            Ok(true) => LinkStatus::Linked,
            _ => LinkStatus::Blocked,
        }
    } else {
        LinkStatus::Blocked
    }
}

/// Create a link from `link_path` to `target`. Returns Ok(true) if created, Ok(false) if skipped.
/// `is_dir` selects directory link (symlink/junction) vs file link (symlink/hardlink).
pub fn create_link(
    link_path: &Path,
    target: &Path,
    label: &str,
    is_dir: bool,
) -> anyhow::Result<bool> {
    match check_link(link_path, target, is_dir) {
        LinkStatus::Linked => {
            println!("  {} {} already linked", "skip".yellow(), label);
            Ok(false)
        }
        LinkStatus::Missing => {
            do_link(target, link_path, is_dir).with_context(|| {
                format!(
                    "Failed to create link: {} → {}",
                    link_path.display(),
                    target.display()
                )
            })?;
            println!("  {} {} → {}", " ok ".green(), label, target.display());
            Ok(true)
        }
        LinkStatus::Broken => {
            // Remove broken link and recreate
            platform::remove_link(link_path)?;
            do_link(target, link_path, is_dir)?;
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
                "  {} {} exists but is not a link, skipping",
                "warn".red(),
                label
            );
            Ok(false)
        }
    }
}

fn do_link(target: &Path, link_path: &Path, is_dir: bool) -> std::io::Result<()> {
    if is_dir {
        platform::link_dir(target, link_path)
    } else {
        platform::link_file(target, link_path)
    }
}

/// Remove a link if it exists. Returns Ok(true) if removed.
/// `is_dir` selects directory-link detection vs file-link detection.
pub fn remove_link(link_path: &Path, label: &str, is_dir: bool) -> anyhow::Result<bool> {
    if link_path.symlink_metadata().is_err() {
        println!("  {} {} not found", "skip".yellow(), label);
        return Ok(false);
    }

    if is_dir {
        if platform::is_dir_link(link_path) {
            platform::remove_link(link_path)?;
            println!("  {} {} removed", " ok ".green(), label);
            Ok(true)
        } else {
            println!("  {} {} is not a link, skipping", "warn".red(), label);
            Ok(false)
        }
    } else {
        // File link: symlink on Unix, hardlink on Windows
        if fs::read_link(link_path).is_ok() {
            // Symlink (Unix)
            platform::remove_link(link_path)?;
            println!("  {} {} removed", " ok ".green(), label);
            Ok(true)
        } else {
            // Not a symlink — on Windows, hardlinks appear as regular files.
            // Safe to remove since this is only called on known managed paths.
            #[cfg(windows)]
            {
                platform::remove_link(link_path)?;
                println!("  {} {} removed", " ok ".green(), label);
                Ok(true)
            }
            #[cfg(not(windows))]
            {
                println!("  {} {} is not a symlink, skipping", "warn".red(), label);
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform;
    use tempfile::TempDir;

    #[test]
    fn test_check_link_missing() {
        let tmp = TempDir::new().unwrap();
        let link = tmp.path().join("link");
        let target = tmp.path().join("target");
        assert_eq!(check_link(&link, &target, true), LinkStatus::Missing);
        assert_eq!(check_link(&link, &target, false), LinkStatus::Missing);
    }

    #[test]
    fn test_check_dir_link_correct() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link");
        platform::link_dir(&target, &link).unwrap();
        assert_eq!(check_link(&link, &target, true), LinkStatus::Linked);
    }

    #[test]
    fn test_check_dir_link_wrong() {
        let tmp = TempDir::new().unwrap();
        let target1 = tmp.path().join("target1");
        let target2 = tmp.path().join("target2");
        fs::create_dir(&target1).unwrap();
        fs::create_dir(&target2).unwrap();
        let link = tmp.path().join("link");
        platform::link_dir(&target1, &link).unwrap();
        assert!(matches!(
            check_link(&link, &target2, true),
            LinkStatus::Wrong(_)
        ));
    }

    #[test]
    fn test_check_file_link_correct() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link");
        platform::link_file(&target, &link).unwrap();
        assert_eq!(check_link(&link, &target, false), LinkStatus::Linked);
    }

    #[test]
    fn test_check_link_blocked() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("file");
        fs::write(&path, "not a link").unwrap();
        let target = tmp.path().join("target");
        assert_eq!(check_link(&path, &target, true), LinkStatus::Blocked);
    }

    #[test]
    fn test_create_dir_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link");
        let created = create_link(&link, &target, "test", true).unwrap();
        assert!(created);
        assert!(platform::is_dir_link(&link));
    }

    #[test]
    fn test_create_file_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link");
        let created = create_link(&link, &target, "test", false).unwrap();
        assert!(created);
        assert_eq!(fs::read_to_string(&link).unwrap(), "content");
    }

    #[test]
    fn test_create_link_idempotent() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link");
        create_link(&link, &target, "test", true).unwrap();
        let created = create_link(&link, &target, "test", true).unwrap();
        assert!(!created); // skipped
    }

    #[test]
    fn test_remove_dir_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link");
        platform::link_dir(&target, &link).unwrap();
        let removed = remove_link(&link, "test", true).unwrap();
        assert!(removed);
        assert!(!platform::is_dir_link(&link));
    }

    #[test]
    fn test_remove_file_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link");
        platform::link_file(&target, &link).unwrap();
        let removed = remove_link(&link, "test", false).unwrap();
        assert!(removed);
        assert!(!link.exists());
        assert!(target.exists());
    }
}
