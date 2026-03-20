use anyhow::Context;
use colored::Colorize;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::platform;

/// Status of a managed file's symlink
#[derive(Debug, PartialEq)]
pub enum FileStatus {
    /// Correct symlink pointing to expected centralized target
    Linked,
    /// Symlink exists but points to wrong target
    Wrong(String),
    /// Neither original nor centralized path exists
    Missing,
    /// Symlink exists but target doesn't exist (broken)
    Broken,
    /// Original is a regular file (not yet migrated)
    Unmanaged,
    /// Original missing but centralized target exists (ready to link)
    ReadyToLink,
}

/// Compute the centralized storage path for a file.
///
/// The centralized path mirrors the file's absolute path under `files_base`:
///   files_base / <absolute_path_without_leading_slash>
pub fn centralized_path(original: &Path, files_base: &Path) -> PathBuf {
    let abs = if original.is_absolute() {
        original.to_path_buf()
    } else {
        // Resolve relative paths from cwd (shouldn't normally happen)
        std::env::current_dir().unwrap_or_default().join(original)
    };
    // Build relative path by filtering out RootDir and Prefix (drive letter) components
    let rel: PathBuf = abs
        .components()
        .filter(|c| !matches!(c, Component::RootDir | Component::Prefix(_)))
        .collect();
    files_base.join(rel)
}

/// Check the status of a managed file
pub fn check_file_status(original: &Path, files_base: &Path) -> FileStatus {
    let central = centralized_path(original, files_base);

    // Check if original path exists as symlink
    match original.symlink_metadata() {
        Err(_) => {
            // Original doesn't exist at all
            if central.exists() {
                FileStatus::ReadyToLink
            } else {
                FileStatus::Missing
            }
        }
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                // It's a symlink — check where it points
                match fs::read_link(original) {
                    Err(_) => FileStatus::Broken,
                    Ok(target) => {
                        let actual = fs::canonicalize(&target)
                            .or_else(|_| {
                                fs::canonicalize(
                                    original.parent().unwrap_or(Path::new(".")).join(&target),
                                )
                            })
                            .unwrap_or(target.clone());
                        let expected =
                            fs::canonicalize(&central).unwrap_or_else(|_| central.clone());

                        if actual == expected {
                            if central.exists() {
                                FileStatus::Linked
                            } else {
                                FileStatus::Broken
                            }
                        } else {
                            FileStatus::Wrong(actual.display().to_string())
                        }
                    }
                }
            } else if central.exists() {
                // Regular file — could be a hardlink (Windows) or unmanaged
                match platform::same_file(original, &central) {
                    Ok(true) => FileStatus::Linked,
                    _ => FileStatus::Unmanaged,
                }
            } else {
                // Regular file or directory — not yet managed
                FileStatus::Unmanaged
            }
        }
    }
}

/// Remove a managed file's link (leaves the central copy intact).
///
/// - Symlink or hardlink to central → remove it
/// - Not a link → warn, skip
/// - Doesn't exist → skip
pub fn unlink_file(original: &Path, files_base: &Path) -> anyhow::Result<bool> {
    let central = centralized_path(original, files_base);
    let label = original.display().to_string();
    match original.symlink_metadata() {
        Err(_) => {
            println!("  {} {} not found", "skip".yellow(), label);
            Ok(false)
        }
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                // Unix symlink
                fs::remove_file(original)?;
                println!("  {} {} removed", " ok ".green(), label);
                Ok(true)
            } else if central.exists() && platform::same_file(original, &central).unwrap_or(false) {
                // Hardlink to central (Windows)
                fs::remove_file(original)?;
                println!("  {} {} removed", " ok ".green(), label);
                Ok(true)
            } else {
                println!(
                    "  {} {} is not a managed link, skipping",
                    "warn".red(),
                    label
                );
                Ok(false)
            }
        }
    }
}

/// Link (centralize) a managed file.
///
/// Behavior:
/// - Already linked → skip
/// - Broken symlink → repair (recreate)
/// - Wrong symlink → warn (or re-link if `yes`)
/// - Regular file, central missing → move to central, create symlink
/// - Regular file, central exists → backup central with timestamp, move, create symlink
/// - Original missing, central exists → create symlink
/// - Neither exists → warn, skip
///
/// Returns Ok(true) if a symlink was created/repaired.
pub fn link_file(original: &Path, files_base: &Path, yes: bool) -> anyhow::Result<bool> {
    let central = centralized_path(original, files_base);
    let label = original.display().to_string();

    match check_file_status(original, files_base) {
        FileStatus::Linked => {
            println!("  {} {} already linked", "skip".yellow(), label);
            Ok(false)
        }

        FileStatus::Broken => {
            // Remove broken symlink and recreate
            fs::remove_file(original)?;
            if !central.exists() {
                println!(
                    "  {} {} broken link and central target missing, skipping",
                    "warn".red(),
                    label
                );
                return Ok(false);
            }
            platform::link_file(&central, original).with_context(|| {
                format!("Failed to create link: {} → {}", label, central.display())
            })?;
            println!("  {} {} (repaired broken link)", " ok ".green(), label);
            Ok(true)
        }

        FileStatus::Wrong(actual) => {
            println!(
                "  {} {} points to {} (expected {})",
                "warn".red(),
                label,
                actual,
                central.display()
            );
            if yes {
                fs::remove_file(original)?;
                platform::link_file(&central, original)?;
                println!("  {} {} re-linked", " ok ".green(), label);
                Ok(true)
            } else {
                Ok(false)
            }
        }

        FileStatus::Unmanaged => {
            // Regular file exists — migrate to central
            if let Some(parent) = central.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create central dir: {}", parent.display())
                })?;
            }

            if central.exists() {
                // Backup existing central file
                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                let backup = central.with_extension(format!("{}.bak", timestamp));
                fs::rename(&central, &backup).with_context(|| {
                    format!("Failed to backup central file: {}", central.display())
                })?;
                println!(
                    "  {} Backed up central file to {}",
                    " ok ".green(),
                    backup.display()
                );
            }

            // Move original → central
            fs::rename(original, &central)
                .with_context(|| format!("Failed to move {} → {}", label, central.display()))?;

            // Create link
            platform::link_file(&central, original).with_context(|| {
                format!("Failed to create link: {} → {}", label, central.display())
            })?;

            println!("  {} {} → {}", " ok ".green(), label, central.display());
            Ok(true)
        }

        FileStatus::ReadyToLink => {
            // Original missing, central exists — just create link
            if let Some(parent) = original.parent() {
                fs::create_dir_all(parent)?;
            }
            platform::link_file(&central, original).with_context(|| {
                format!("Failed to create link: {} → {}", label, central.display())
            })?;
            println!("  {} {} → {}", " ok ".green(), label, central.display());
            Ok(true)
        }

        FileStatus::Missing => {
            println!(
                "  {} {} not found (neither original nor central)",
                "skip".yellow(),
                label
            );
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, PathBuf, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let files_base = tmp.path().join("files_base");
        let tool_dir = tmp.path().join("tool");
        fs::create_dir_all(&tool_dir).unwrap();
        (tmp, files_base, tool_dir)
    }

    #[test]
    fn test_centralized_path() {
        let tmp = TempDir::new().unwrap();
        let files_base = tmp.path().join("agm").join("files");
        let original = tmp
            .path()
            .join("Users")
            .join("wen")
            .join(".claude")
            .join("settings.json");
        let central = centralized_path(&original, &files_base);
        // The centralized path should strip root/prefix and join under files_base
        assert!(central.starts_with(&files_base));
        assert!(central.ends_with("settings.json"));
    }

    #[test]
    fn test_check_status_missing() {
        let (_tmp, files_base, tool_dir) = setup();
        let original = tool_dir.join("nonexistent.json");
        assert_eq!(
            check_file_status(&original, &files_base),
            FileStatus::Missing
        );
    }

    #[test]
    fn test_check_status_ready_to_link() {
        let (_tmp, files_base, tool_dir) = setup();
        let original = tool_dir.join("settings.json");
        // Create central file but not original
        let central = centralized_path(&original, &files_base);
        fs::create_dir_all(central.parent().unwrap()).unwrap();
        fs::write(&central, "{}").unwrap();
        assert_eq!(
            check_file_status(&original, &files_base),
            FileStatus::ReadyToLink
        );
    }

    #[test]
    fn test_check_status_unmanaged() {
        let (_tmp, files_base, tool_dir) = setup();
        let original = tool_dir.join("settings.json");
        fs::write(&original, "{}").unwrap();
        assert_eq!(
            check_file_status(&original, &files_base),
            FileStatus::Unmanaged
        );
    }

    #[test]
    fn test_link_file_unmanaged_no_central() {
        let (_tmp, files_base, tool_dir) = setup();
        let original = tool_dir.join("settings.json");
        fs::write(&original, r#"{"key": "value"}"#).unwrap();

        let result = link_file(&original, &files_base, false).unwrap();
        assert!(result);
        // Original should now be linked to central
        let central = centralized_path(&original, &files_base);
        assert_eq!(fs::read_to_string(&central).unwrap(), r#"{"key": "value"}"#);
        // Link should be readable through original path
        assert_eq!(
            fs::read_to_string(&original).unwrap(),
            r#"{"key": "value"}"#
        );
    }

    #[test]
    fn test_link_file_unmanaged_central_exists_creates_backup() {
        let (_tmp, files_base, tool_dir) = setup();
        let original = tool_dir.join("settings.json");
        fs::write(&original, "new content").unwrap();

        // Pre-populate central
        let central = centralized_path(&original, &files_base);
        fs::create_dir_all(central.parent().unwrap()).unwrap();
        fs::write(&central, "old content").unwrap();

        link_file(&original, &files_base, false).unwrap();

        // Central now has new content
        assert_eq!(fs::read_to_string(&central).unwrap(), "new content");
        // A .bak file should exist
        let parent = central.parent().unwrap();
        let bak_count = fs::read_dir(parent)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains(".bak")
            })
            .count();
        assert_eq!(bak_count, 1);
    }

    #[test]
    fn test_link_file_ready_to_link() {
        let (_tmp, files_base, tool_dir) = setup();
        let original = tool_dir.join("settings.json");
        let central = centralized_path(&original, &files_base);
        fs::create_dir_all(central.parent().unwrap()).unwrap();
        fs::write(&central, "content").unwrap();

        let result = link_file(&original, &files_base, false).unwrap();
        assert!(result);
        assert!(original.exists());
    }

    #[test]
    fn test_link_file_already_linked() {
        let (_tmp, files_base, tool_dir) = setup();
        let original = tool_dir.join("settings.json");
        let central = centralized_path(&original, &files_base);
        fs::create_dir_all(central.parent().unwrap()).unwrap();
        fs::write(&central, "content").unwrap();
        platform::link_file(&central, &original).unwrap();

        let result = link_file(&original, &files_base, false).unwrap();
        assert!(!result); // skipped
    }

    #[test]
    fn test_link_file_broken_skip_no_central() {
        // Broken link pointing to central, but central doesn't exist → skip
        // (This scenario only applies on Unix where symlinks can be broken)
        #[cfg(unix)]
        {
            let (_tmp, files_base, tool_dir) = setup();
            let original = tool_dir.join("settings.json");
            let central = centralized_path(&original, &files_base);
            fs::create_dir_all(central.parent().unwrap()).unwrap();

            // Create symlink pointing to central (central doesn't exist → broken)
            platform::link_file(&central, &original).unwrap();

            let result = link_file(&original, &files_base, false).unwrap();
            // Cannot repair: central has no content, returns false
            assert!(!result);
        }
    }

    #[test]
    fn test_link_file_broken_repaired_when_central_recreated() {
        // Simulate: was linked, central deleted, then central re-created before link_file runs.
        // On Unix (symlinks): symlink still points to central path → Linked → skip
        // On Windows (hardlinks): original still has old content, new central is a different
        //   file → Unmanaged → re-link
        let (_tmp, files_base, tool_dir) = setup();
        let original = tool_dir.join("settings.json");
        let central = centralized_path(&original, &files_base);
        fs::create_dir_all(central.parent().unwrap()).unwrap();
        fs::write(&central, "content").unwrap();
        platform::link_file(&central, &original).unwrap();
        // Delete and recreate central (simulates transient deletion)
        fs::remove_file(&central).unwrap();
        fs::write(&central, "repaired").unwrap();

        let result = link_file(&original, &files_base, false).unwrap();
        #[cfg(unix)]
        {
            // Symlink still valid → skip
            assert!(!result);
            assert_eq!(fs::read_to_string(&original).unwrap(), "repaired");
        }
        #[cfg(windows)]
        {
            // Hardlink broken (different inode) → re-linked
            assert!(result);
            // After re-link, original is readable
            assert!(original.exists());
        }
    }
}
