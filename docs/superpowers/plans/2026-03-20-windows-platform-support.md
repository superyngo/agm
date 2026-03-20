# Windows Platform Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add full Windows platform support to AGM, using NTFS junctions (directories) and hardlinks (files) as the link mechanism on Windows, while keeping Unix symlink behavior unchanged.

**Architecture:** A new `src/platform.rs` module encapsulates all platform-specific link operations behind a unified API. Conditional compilation (`#[cfg(unix)]`/`#[cfg(windows)]`) is isolated within `platform.rs`. All other modules call `platform::*` instead of `std::os::unix::fs` directly. Path handling is updated for Windows formats. CI/CD adds Windows build targets.

**Tech Stack:** Rust, `junction` crate (Windows junctions), `std::fs::hard_link` (cross-platform hardlinks), `std::os::windows::fs::MetadataExt` (file identity comparison)

**Spec:** `docs/superpowers/specs/2026-03-20-windows-platform-support-design.md`

---

## File Structure

### New Files
- `src/platform.rs` — Platform abstraction layer (link ops, editor default, capability check)

### Modified Files
- `Cargo.toml` — Add `junction` Windows-only dependency
- `src/main.rs` — Add `mod platform`, migrate all `unix_fs` and `is_symlink` calls
- `src/linker.rs` — Migrate to `platform::*`, add `is_dir` parameter to `check_link` and `create_link`
- `src/files.rs` — Migrate to `platform::*`, fix `centralized_path()` for Windows, update `unlink_file` signature
- `src/skills.rs` — Migrate all `unix_fs` and `is_symlink` calls to `platform::*`
- `src/status.rs` — Update `check_link` calls with `is_dir` argument
- `src/config.rs` — Fix `resolve_path()` for Windows path detection
- `src/editor.rs` — Use `platform::default_editor()` fallback
- `src/init.rs` — Add link capability detection
- `.github/workflows/release.yml` — Add Windows build matrix + zip packaging
- `README.md` — Add Windows installation section
- `RELEASE.md` — Add Windows platform info

---

### Task 1: Create `platform.rs` and update `Cargo.toml`

**Files:**
- Create: `src/platform.rs`
- Modify: `Cargo.toml` (add junction dependency)
- Modify: `src/main.rs:1-8` (add `mod platform`)

- [ ] **Step 1: Add `junction` dependency to `Cargo.toml`**

Add Windows-only dependency after the `[dev-dependencies]` section:

```toml
[target.'cfg(windows)'.dependencies]
junction = "1"
```

- [ ] **Step 2: Write `platform.rs` tests first**

Create `src/platform.rs` with the public API signatures, the `#[cfg(test)]` module, and stub implementations that `todo!()`. This lets us see the test design first.

```rust
//! Platform abstraction for link operations.
//!
//! Unix: uses symlinks for both files and directories.
//! Windows: uses NTFS junctions for directories and hardlinks for files.

use std::io;
use std::path::{Path, PathBuf};

pub enum LinkCapability {
    Full,
    Limited(String),
    Unavailable(String),
}

pub fn link_dir(target: &Path, link_path: &Path) -> io::Result<()> { sys::link_dir(target, link_path) }
pub fn link_file(target: &Path, link_path: &Path) -> io::Result<()> { sys::link_file(target, link_path) }
pub fn remove_link(link_path: &Path) -> io::Result<()> { sys::remove_link(link_path) }
pub fn is_dir_link(path: &Path) -> bool { sys::is_dir_link(path) }
pub fn read_dir_link_target(path: &Path) -> Option<PathBuf> { sys::read_dir_link_target(path) }
pub fn same_file(a: &Path, b: &Path) -> io::Result<bool> { sys::same_file(a, b) }
pub fn default_editor() -> &'static str { sys::DEFAULT_EDITOR }
pub fn check_link_capability() -> LinkCapability { sys::check_link_capability() }

#[cfg(unix)]    mod sys { /* will implement next */ }
#[cfg(windows)] mod sys { /* will implement next */ }

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_link_dir_and_read_target() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target_dir");
        fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link_dir");

        link_dir(&target, &link).unwrap();
        assert!(is_dir_link(&link));

        let read_target = read_dir_link_target(&link).unwrap();
        let canonical_target = fs::canonicalize(&target).unwrap();
        let canonical_read = fs::canonicalize(&read_target).unwrap();
        assert_eq!(canonical_read, canonical_target);
    }

    #[test]
    fn test_link_file_and_same_file() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target.txt");
        fs::write(&target, "hello").unwrap();
        let link = tmp.path().join("link.txt");

        link_file(&target, &link).unwrap();
        assert!(same_file(&link, &target).unwrap());
        assert_eq!(fs::read_to_string(&link).unwrap(), "hello");
    }

    #[test]
    fn test_remove_dir_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target_dir");
        fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link_dir");
        link_dir(&target, &link).unwrap();

        remove_link(&link).unwrap();
        assert!(!link.exists());
        assert!(target.exists());
    }

    #[test]
    fn test_remove_file_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target.txt");
        fs::write(&target, "hello").unwrap();
        let link = tmp.path().join("link.txt");
        link_file(&target, &link).unwrap();

        remove_link(&link).unwrap();
        assert!(!link.exists());
        assert!(target.exists());
    }

    #[test]
    fn test_is_dir_link_false_for_regular_dir() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("regular_dir");
        fs::create_dir(&dir).unwrap();
        assert!(!is_dir_link(&dir));
    }

    #[test]
    fn test_same_file_different_files() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        assert!(!same_file(&a, &b).unwrap());
    }

    #[test]
    fn test_default_editor_not_empty() {
        assert!(!default_editor().is_empty());
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test platform::tests`
Expected: FAIL (compilation error — `sys` module has no implementations)

- [ ] **Step 4: Implement Unix `sys` module**

Replace the `#[cfg(unix)] mod sys` stub with:

```rust
#[cfg(unix)]
mod sys {
    use super::*;
    use std::fs;
    use std::os::unix::fs as unix_fs;

    pub const DEFAULT_EDITOR: &str = "vi";

    pub fn link_dir(target: &Path, link_path: &Path) -> io::Result<()> {
        unix_fs::symlink(target, link_path)
    }

    pub fn link_file(target: &Path, link_path: &Path) -> io::Result<()> {
        unix_fs::symlink(target, link_path)
    }

    pub fn remove_link(link_path: &Path) -> io::Result<()> {
        fs::remove_file(link_path)
    }

    pub fn is_dir_link(path: &Path) -> bool {
        path.symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    }

    pub fn read_dir_link_target(path: &Path) -> Option<PathBuf> {
        fs::read_link(path).ok()
    }

    pub fn same_file(a: &Path, b: &Path) -> io::Result<bool> {
        use std::os::unix::fs::MetadataExt;
        let ma = fs::metadata(a)?;
        let mb = fs::metadata(b)?;
        Ok(ma.dev() == mb.dev() && ma.ino() == mb.ino())
    }

    pub fn check_link_capability() -> LinkCapability {
        LinkCapability::Full
    }
}
```

- [ ] **Step 5: Implement Windows `sys` module**

Replace the `#[cfg(windows)] mod sys` stub with:

```rust
#[cfg(windows)]
mod sys {
    use super::*;
    use std::fs;

    pub const DEFAULT_EDITOR: &str = "notepad";

    pub fn link_dir(target: &Path, link_path: &Path) -> io::Result<()> {
        junction::create(target, link_path)
    }

    pub fn link_file(target: &Path, link_path: &Path) -> io::Result<()> {
        fs::hard_link(target, link_path)
    }

    pub fn remove_link(link_path: &Path) -> io::Result<()> {
        if is_dir_link(link_path) {
            fs::remove_dir(link_path)
        } else {
            fs::remove_file(link_path)
        }
    }

    pub fn is_dir_link(path: &Path) -> bool {
        junction::exists(path).unwrap_or(false)
    }

    pub fn read_dir_link_target(path: &Path) -> Option<PathBuf> {
        if is_dir_link(path) {
            junction::get_target(path).ok()
        } else {
            None
        }
    }

    pub fn same_file(a: &Path, b: &Path) -> io::Result<bool> {
        use std::os::windows::fs::MetadataExt;
        let ma = fs::metadata(a)?;
        let mb = fs::metadata(b)?;
        match (
            ma.volume_serial_number(),
            ma.file_index(),
            mb.volume_serial_number(),
            mb.file_index(),
        ) {
            (Some(va), Some(ia), Some(vb), Some(ib)) => Ok(va == vb && ia == ib),
            _ => {
                // Fallback: compare canonical paths
                let ca = fs::canonicalize(a)?;
                let cb = fs::canonicalize(b)?;
                Ok(ca == cb)
            }
        }
    }

    pub fn check_link_capability() -> LinkCapability {
        let tmp = std::env::temp_dir().join("agm_capability_check");
        let _ = fs::remove_dir_all(&tmp);
        if fs::create_dir_all(&tmp).is_err() {
            return LinkCapability::Unavailable("Cannot create temp directory".into());
        }

        let junc_target = tmp.join("junc_target");
        let junc_link = tmp.join("junc_link");
        let _ = fs::create_dir(&junc_target);
        let junc_ok = junction::create(&junc_target, &junc_link).is_ok();

        let hl_src = tmp.join("hl_src.txt");
        let hl_dst = tmp.join("hl_dst.txt");
        let _ = fs::write(&hl_src, "test");
        let hl_ok = fs::hard_link(&hl_src, &hl_dst).is_ok();

        let _ = fs::remove_dir_all(&tmp);

        match (junc_ok, hl_ok) {
            (true, true) => LinkCapability::Full,
            (true, false) => LinkCapability::Limited("Cannot create hardlinks".into()),
            (false, true) => LinkCapability::Limited("Cannot create junctions".into()),
            (false, false) => LinkCapability::Unavailable(
                "Cannot create junctions or hardlinks. Check NTFS permissions.".into(),
            ),
        }
    }
}
```

- [ ] **Step 6: Add `mod platform` to `main.rs`**

In `src/main.rs`, add `mod platform;` after the existing module declarations (line 8):

```rust
mod config;
mod editor;
mod files;
mod init;
mod linker;
mod paths;
mod platform;
mod skills;
mod status;
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test platform::tests`
Expected: All 7 tests PASS

- [ ] **Step 8: Commit**

```bash
git add src/platform.rs src/main.rs Cargo.toml Cargo.lock
git commit -m "feat: add platform abstraction layer for Windows support

Introduce src/platform.rs with unified API for link operations:
- Unix: symlinks (unchanged behavior)
- Windows: NTFS junctions (dirs) + hardlinks (files)

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 2: Migrate `linker.rs`

**Files:**
- Modify: `src/linker.rs` (full rewrite of functions)
- Test: existing tests in `src/linker.rs` (adapted)

The key changes:
1. Remove `use std::os::unix::fs as unix_fs`
2. Add `is_dir: bool` parameter to `check_link` and `create_link`
3. Split `check_link` into dir-link and file-link paths
4. Use `platform::*` for all link operations

- [ ] **Step 1: Rewrite `linker.rs`**

Replace the entire `src/linker.rs` with:

```rust
use anyhow::Context;
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::platform;

#[derive(Debug, PartialEq)]
pub enum LinkStatus {
    Linked,
    Wrong(String),
    Blocked,
    Missing,
    Broken,
}

/// Check the status of a link at `link_path` that should point to `expected_target`.
/// `is_dir` indicates whether this is a directory link (skills) or file link (prompt).
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

fn check_dir_link(link_path: &Path, expected_target: &Path) -> LinkStatus {
    match platform::read_dir_link_target(link_path) {
        Some(actual_target) => {
            let actual = fs::canonicalize(&actual_target)
                .or_else(|_| {
                    fs::canonicalize(
                        link_path
                            .parent()
                            .unwrap_or(Path::new("."))
                            .join(&actual_target),
                    )
                })
                .unwrap_or(actual_target);
            let expected =
                fs::canonicalize(expected_target).unwrap_or_else(|_| expected_target.to_path_buf());

            if actual == expected {
                if expected_target.exists() {
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

fn check_file_link(link_path: &Path, expected_target: &Path) -> LinkStatus {
    // Try read_link first — works for Unix symlinks
    if let Ok(actual_target) = fs::read_link(link_path) {
        let actual = fs::canonicalize(&actual_target)
            .or_else(|_| {
                fs::canonicalize(
                    link_path
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join(&actual_target),
                )
            })
            .unwrap_or(actual_target);
        let expected =
            fs::canonicalize(expected_target).unwrap_or_else(|_| expected_target.to_path_buf());

        if actual == expected {
            if expected_target.exists() {
                LinkStatus::Linked
            } else {
                LinkStatus::Broken
            }
        } else {
            LinkStatus::Wrong(actual.display().to_string())
        }
    } else {
        // Not a symlink — check if it's a hardlink (Windows) via same_file
        if !expected_target.exists() {
            return LinkStatus::Blocked;
        }
        match platform::same_file(link_path, expected_target) {
            Ok(true) => LinkStatus::Linked,
            Ok(false) => LinkStatus::Blocked,
            Err(_) => LinkStatus::Blocked,
        }
    }
}

/// Create a link from `link_path` to `target`.
/// `is_dir` selects directory link (junction on Windows) vs file link (hardlink on Windows).
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

/// Remove a link if it exists.
pub fn remove_link(link_path: &Path, label: &str) -> anyhow::Result<bool> {
    if link_path.symlink_metadata().is_err() {
        println!("  {} {} not found", "skip".yellow(), label);
        return Ok(false);
    }

    // Determine if this is a removable link
    let is_removable_link = platform::is_dir_link(link_path)
        || link_path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);

    // On Windows, hardlinks appear as regular files
    #[cfg(windows)]
    let is_removable_link = is_removable_link || link_path.is_file();

    if is_removable_link {
        platform::remove_link(link_path)?;
        println!("  {} {} removed", " ok ".green(), label);
        Ok(true)
    } else {
        println!("  {} {} is not a link, skipping", "warn".red(), label);
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform;
    use std::fs;
    use tempfile::TempDir;

    // --- Directory link tests ---

    #[test]
    fn test_check_dir_link_missing() {
        let tmp = TempDir::new().unwrap();
        let link = tmp.path().join("link");
        let target = tmp.path().join("target");
        assert_eq!(check_link(&link, &target, true), LinkStatus::Missing);
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
        assert!(matches!(check_link(&link, &target2, true), LinkStatus::Wrong(_)));
    }

    #[test]
    fn test_check_dir_link_blocked() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("regular_dir");
        fs::create_dir(&dir).unwrap();
        let target = tmp.path().join("target");
        assert_eq!(check_link(&dir, &target, true), LinkStatus::Blocked);
    }

    // --- File link tests ---

    #[test]
    fn test_check_file_link_missing() {
        let tmp = TempDir::new().unwrap();
        let link = tmp.path().join("link");
        let target = tmp.path().join("target");
        assert_eq!(check_link(&link, &target, false), LinkStatus::Missing);
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
    fn test_check_file_link_blocked() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("file");
        fs::write(&path, "not a link").unwrap();
        let target = tmp.path().join("target");
        assert_eq!(check_link(&path, &target, false), LinkStatus::Blocked);
    }

    // --- Create/remove tests ---

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
        assert!(platform::same_file(&link, &target).unwrap());
    }

    #[test]
    fn test_create_link_idempotent() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link");
        create_link(&link, &target, "test", false).unwrap();
        let created = create_link(&link, &target, "test", false).unwrap();
        assert!(!created); // skipped
    }

    #[test]
    fn test_remove_dir_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link");
        platform::link_dir(&target, &link).unwrap();
        let removed = remove_link(&link, "test").unwrap();
        assert!(removed);
        assert!(!link.exists());
    }

    #[test]
    fn test_remove_file_link() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, "content").unwrap();
        let link = tmp.path().join("link");
        platform::link_file(&target, &link).unwrap();
        let removed = remove_link(&link, "test").unwrap();
        assert!(removed);
        assert!(!link.exists());
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test linker::tests`
Expected: All 11 tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/linker.rs
git commit -m "refactor: migrate linker.rs to platform abstraction

- Replace unix_fs::symlink with platform::link_dir/link_file
- Add is_dir parameter to check_link and create_link
- Split check_link into dir-link and file-link paths
- Use platform::same_file for hardlink verification (Windows)

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 3: Migrate `files.rs`

**Files:**
- Modify: `src/files.rs` (migrate to platform, fix centralized_path)

Key changes:
1. Remove `use std::os::unix::fs as unix_fs`
2. Fix `centralized_path()` to handle Windows drive prefixes
3. Rewrite `check_file_status()` to work with both symlinks and hardlinks
4. Replace `unix_fs::symlink` with `platform::link_file` in `link_file()`
5. Update `unlink_file()` to detect hardlinks (add `files_base` parameter)

- [ ] **Step 1: Fix `centralized_path()` for cross-platform paths**

Replace the `centralized_path` function (around line 28-38):

```rust
use std::path::Component;

pub fn centralized_path(original: &Path, files_base: &Path) -> PathBuf {
    let abs = if original.is_absolute() {
        original.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(original)
    };
    // Strip root components (/ on Unix, C:\ on Windows) to get relative path
    let rel: PathBuf = abs
        .components()
        .filter(|c| !matches!(c, Component::Prefix(_) | Component::RootDir))
        .collect();
    files_base.join(rel)
}
```

- [ ] **Step 2: Rewrite `check_file_status()`**

Replace lines 41-87 with a cross-platform implementation:

```rust
pub fn check_file_status(original: &Path, files_base: &Path) -> FileStatus {
    let central = centralized_path(original, files_base);
    let central_exists = central.exists();

    match original.symlink_metadata() {
        Err(_) => {
            if central_exists {
                FileStatus::ReadyToLink
            } else {
                FileStatus::Missing
            }
        }
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                // Unix symlink — check where it points
                check_symlink_file_status(original, &central, central_exists)
            } else if central_exists {
                // Regular file or hardlink — compare with central
                match crate::platform::same_file(original, &central) {
                    Ok(true) => FileStatus::Linked,
                    _ => FileStatus::Unmanaged,
                }
            } else {
                FileStatus::Unmanaged
            }
        }
    }
}

fn check_symlink_file_status(original: &Path, central: &Path, central_exists: bool) -> FileStatus {
    match fs::read_link(original) {
        Err(_) => FileStatus::Broken,
        Ok(target) => {
            let actual = fs::canonicalize(&target)
                .or_else(|_| {
                    fs::canonicalize(original.parent().unwrap_or(Path::new(".")).join(&target))
                })
                .unwrap_or(target);
            let expected = fs::canonicalize(central).unwrap_or_else(|_| central.to_path_buf());

            if actual == expected {
                if central_exists {
                    FileStatus::Linked
                } else {
                    FileStatus::Broken
                }
            } else {
                FileStatus::Wrong(actual.display().to_string())
            }
        }
    }
}
```

- [ ] **Step 3: Migrate `link_file()` and update `unlink_file()`**

In `link_file()`, replace all 4 occurrences of `unix_fs::symlink(&central, original)` with `crate::platform::link_file(&central, original)`.

Also replace `fs::remove_file(original)?;` calls that remove broken/wrong symlinks before re-creating with `crate::platform::remove_link(original)?;`.

Update `unlink_file` signature to accept `files_base` for hardlink detection:

```rust
pub fn unlink_file(original: &Path, files_base: &Path) -> anyhow::Result<bool> {
    let label = original.display().to_string();
    match original.symlink_metadata() {
        Err(_) => {
            println!("  {} {} not found", "skip".yellow(), label);
            Ok(false)
        }
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                crate::platform::remove_link(original)?;
                println!("  {} {} removed", " ok ".green(), label);
                Ok(true)
            } else {
                // Check if it's a hardlink to central (Windows)
                let central = centralized_path(original, files_base);
                if central.exists() {
                    if let Ok(true) = crate::platform::same_file(original, &central) {
                        crate::platform::remove_link(original)?;
                        println!("  {} {} removed", " ok ".green(), label);
                        return Ok(true);
                    }
                }
                println!("  {} {} is not a managed link, skipping", "warn".red(), label);
                Ok(false)
            }
        }
    }
}
```

- [ ] **Step 4: Remove the `use std::os::unix::fs as unix_fs;` import (line 4)**

Replace with:

```rust
use std::path::Component;
```

- [ ] **Step 5: Update tests to use platform functions**

Replace `unix_fs::symlink` in test code with `crate::platform::link_file`.
Replace `.is_symlink()` assertions with `crate::platform::same_file()` checks.
Update `unlink_file` test calls to pass `files_base`.

For example, the `test_link_file_already_linked` test:
```rust
#[test]
fn test_link_file_already_linked() {
    let (_tmp, files_base, tool_dir) = setup();
    let original = tool_dir.join("settings.json");
    let central = centralized_path(&original, &files_base);
    fs::create_dir_all(central.parent().unwrap()).unwrap();
    fs::write(&central, "content").unwrap();
    crate::platform::link_file(&central, &original).unwrap();

    let result = link_file(&original, &files_base, false).unwrap();
    assert!(!result); // skipped
}
```

Update all `unlink_file(&original)?` calls in tests to `unlink_file(&original, &files_base)?`.

- [ ] **Step 6: Run tests**

Run: `cargo test files::tests`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/files.rs
git commit -m "refactor: migrate files.rs to platform abstraction

- Fix centralized_path for Windows drive prefixes
- Use platform::same_file for hardlink detection
- Replace unix_fs::symlink with platform::link_file
- Update unlink_file to handle hardlinks via files_base param

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 4: Migrate `skills.rs`

**Files:**
- Modify: `src/skills.rs`

Replace all Unix-specific calls with platform abstractions. Skills are directories, so they use `platform::link_dir`.

- [ ] **Step 1: Migrate production code**

Line 4: Remove `use std::os::unix::fs as unix_fs;`

Line 67-72 (`list_skills`): Replace `path.is_symlink()` with `platform::is_dir_link(&path)`:
```rust
if platform::is_dir_link(&path) || path.is_dir() {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        let source = platform::read_dir_link_target(&path).unwrap_or(path.clone());
        skills.push((name.to_string(), source));
    }
}
```

Line 107 (`add_local`): Replace `unix_fs::symlink(&skill_path, &link_path)` with:
```rust
platform::link_dir(&skill_path, &link_path)
```

Line 129 (`remove_skill`): Replace `if skill_path.is_symlink()` with `if platform::is_dir_link(&skill_path)`:
```rust
if platform::is_dir_link(&skill_path) {
    platform::remove_link(&skill_path)?;
    println!("{} Removed skill: {}", " ok ".green(), name);
}
```

Line 156 (`prune_broken_skills`): Replace `if path.is_symlink()` with `if platform::is_dir_link(&path)`:
```rust
if platform::is_dir_link(&path) {
    if !path.exists() {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("<unknown>");
        platform::remove_link(&path)?;
        println!("  {} {} (broken skill link removed)", "warn".yellow(), name);
        removed += 1;
    }
}
```

Line 237-239 (`update_all`): Replace `if link_path.is_symlink()` with `if platform::is_dir_link(&link_path)`.
Also replace `fs::read_link(&link_path)` (line 239) with `platform::read_dir_link_target(&link_path)` for consistency with `list_skills`.

Add `use crate::platform;` at the top.

- [ ] **Step 2: Update tests**

Replace `unix_fs::symlink` with `platform::link_dir` in all test functions.
Replace `.is_symlink()` assertions with `platform::is_dir_link()`.

- [ ] **Step 3: Run tests**

Run: `cargo test skills::tests`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/skills.rs
git commit -m "refactor: migrate skills.rs to platform abstraction

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 5: Migrate `main.rs`

**Files:**
- Modify: `src/main.rs` (multiple locations)

- [ ] **Step 1: Migrate `migrate_skills_dir` (line 170-250)**

Line 177: Remove `use std::os::unix::fs as unix_fs;`
Line 229: Replace `fs::remove_file(&link)?` with `platform::remove_link(&link)?` (stale link removal — `fs::remove_file` fails on junctions)
Line 232: Replace `unix_fs::symlink(&dest, &link)` with `platform::link_dir(&dest, &link)`

- [ ] **Step 2: Migrate `copy_dir_all` (line 252-275)**

Line 260-266: Replace the symlink branch:
```rust
if meta.file_type().is_symlink() {
    if let Ok(target) = fs::read_link(&src_path) {
        if dst_path.symlink_metadata().is_ok() {
            fs::remove_file(&dst_path)?;
        }
        if src_path.is_dir() {
            platform::link_dir(&target, &dst_path)?;
        } else {
            platform::link_file(&target, &dst_path)?;
        }
    }
}
```

Also handle junctions on Windows — add a check for `platform::is_dir_link`:
```rust
if meta.file_type().is_symlink() || platform::is_dir_link(&src_path) {
    // Recreate the link at destination
    let target = if platform::is_dir_link(&src_path) {
        platform::read_dir_link_target(&src_path)
    } else {
        fs::read_link(&src_path).ok()
    };
    if let Some(target) = target {
        if dst_path.symlink_metadata().is_ok() {
            let _ = platform::remove_link(&dst_path);
        }
        if src_path.is_dir() {
            platform::link_dir(&target, &dst_path)?;
        } else {
            platform::link_file(&target, &dst_path)?;
        }
    }
} else if meta.is_dir() {
```

- [ ] **Step 3: Update `link` command callers (line 446-576)**

All calls to `linker::create_link` need the new `is_dir` parameter:

Line 507 (`linker::create_link(&skills_link, &central_skills, "skills")`):
→ `linker::create_link(&skills_link, &central_skills, "skills", true)`

Line 565 (`linker::create_link(&prompt_link, &central_prompt, "prompt")`):
→ `linker::create_link(&prompt_link, &central_prompt, "prompt", false)`

- [ ] **Step 4: Update `link` command — existing link detection (line 453-478, 514-539)**

Lines 453: Replace `if skills_link.is_symlink()` with:
```rust
if skills_link.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false)
    || platform::is_dir_link(&skills_link)
```

Lines 454: Replace `let actual_target = fs::read_link(&skills_link)?` with:
```rust
let actual_target = platform::read_dir_link_target(&skills_link)
    .or_else(|| fs::read_link(&skills_link).ok())
    .ok_or_else(|| anyhow::anyhow!("Cannot read link target"))?;
```

Lines 514-515: Same pattern for prompt link detection. Since prompts are files on Windows (hardlinks), the `is_symlink()` check is still correct for Unix. On Windows, hardlinks aren't detected here — they'll fall through to the `else if prompt_link.exists()` branch at line 540, which handles regular files (backs up and re-links). This is acceptable behavior.

- [ ] **Step 5: Update `unlink` command callers (line 609, 670)**

All calls to `files::unlink_file(&original)` gain the `files_base` argument:

Line 609: `files::unlink_file(&original)?` → `files::unlink_file(&original, &files_base)?`
Line 670: `files::unlink_file(&original)?` → `files::unlink_file(&original, &files_base)?`
Line 688: `files::unlink_file(&original)?` → `files::unlink_file(&original, &files_base)?`

- [ ] **Step 6: Update `unlink` command — skills copy-back (line 650-651)**

After removing a junction on Windows, `copy_dir_all` copies skills back. The existing `copy_dir_all` now uses platform functions (from Step 2), so this works.

- [ ] **Step 7: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 8: Commit**

```bash
git add src/main.rs
git commit -m "refactor: migrate main.rs to platform abstraction

- Update migrate_skills_dir and copy_dir_all for cross-platform links
- Pass is_dir to create_link calls
- Pass files_base to unlink_file calls
- Handle junction detection in link command

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 6: Migrate `status.rs`, `config.rs`, `editor.rs`, `init.rs`

**Files:**
- Modify: `src/status.rs` (update `check_link` calls)
- Modify: `src/config.rs` (fix `resolve_path`)
- Modify: `src/editor.rs` (use `platform::default_editor`)
- Modify: `src/init.rs` (add capability check)

- [ ] **Step 1: Update `status.rs`**

Line 33-34 (prompt check): Add `false` for `is_dir`:
```rust
Some(check_link(&link, &central_prompt, false))
```

Line 39-40 (skills check): Add `true` for `is_dir`:
```rust
Some(check_link(&link, &central_skills, true))
```

- [ ] **Step 2: Fix `config.rs` `resolve_path` (line 186-192)**

Replace:
```rust
pub fn resolve_path(&self, path: &str) -> PathBuf {
    if path.contains('/') {
        expand_path(path)
    } else {
        self.resolved_config_dir().join(path)
    }
}
```

With:
```rust
pub fn resolve_path(&self, path: &str) -> PathBuf {
    if is_absolute_or_rooted(path) {
        expand_path(path)
    } else {
        self.resolved_config_dir().join(path)
    }
}
```

Add helper function (outside impl block, or as a free function in the module):
```rust
/// Check if a path string looks like an absolute or rooted path
/// (contains path separators, starts with ~, or has a Windows drive letter).
fn is_absolute_or_rooted(path: &str) -> bool {
    path.contains('/')
        || path.contains('\\')
        || path.starts_with('~')
        || (path.len() >= 2 && path.as_bytes()[1] == b':')
}
```

- [ ] **Step 3: Fix `editor.rs` (line 7-9)**

Replace:
```rust
pub fn get_editor(config: &Config) -> String {
    if !config.editor.is_empty() {
        return config.editor.clone();
    }
    std::env::var("EDITOR").unwrap_or_else(|_| "vi".into())
}
```

With:
```rust
pub fn get_editor(config: &Config) -> String {
    if !config.editor.is_empty() {
        return config.editor.clone();
    }
    std::env::var("EDITOR").unwrap_or_else(|_| crate::platform::default_editor().into())
}
```

- [ ] **Step 4: Add capability check to `init.rs`**

At the end of the `run()` function (before the final `Ok(())`), add:

```rust
// Check link capability (informational)
#[cfg(windows)]
{
    use crate::platform;
    match platform::check_link_capability() {
        platform::LinkCapability::Full => {
            println!("\n{} NTFS junction + hardlink support detected", " ok ".green());
        }
        platform::LinkCapability::Limited(msg) => {
            println!("\n{} Limited link support: {}", "warn".yellow(), msg);
        }
        platform::LinkCapability::Unavailable(msg) => {
            println!("\n{} {}", "warn".red(), msg);
            println!("  AGM link/unlink commands may not work correctly.");
        }
    }
}
```

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/status.rs src/config.rs src/editor.rs src/init.rs
git commit -m "refactor: migrate status/config/editor/init for Windows support

- Pass is_dir to check_link in status.rs
- Fix resolve_path to detect Windows paths (backslash, drive letter)
- Use platform::default_editor() fallback
- Add NTFS capability check on Windows init

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 7: Update CI/CD (`release.yml`)

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Add Windows matrix entries**

In the `matrix.include` array (after the macOS entries), add:

```yaml
          # Windows builds
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact_name: agm.exe
            asset_name: agm-windows-x86_64
          - os: windows-latest
            target: i686-pc-windows-msvc
            artifact_name: agm.exe
            asset_name: agm-windows-i686
```

- [ ] **Step 2: Add Windows build step**

After the "Build with cargo (simple targets)" step, add:

```yaml
      - name: Build (Windows)
        if: runner.os == 'Windows'
        run: cargo build --release --target ${{ matrix.target }}

      - name: Run tests (Windows)
        if: runner.os == 'Windows'
        run: cargo test --target ${{ matrix.target }}
```

- [ ] **Step 3: Add Windows packaging step**

After the "Create tarball" step, add:

```yaml
      - name: Create zip (Windows)
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          Compress-Archive -Path "target\${{ matrix.target }}\release\${{ matrix.artifact_name }}" -DestinationPath "${{ matrix.asset_name }}.zip"
```

- [ ] **Step 4: Update artifact upload to handle both formats**

Modify the upload step to handle both .tar.gz and .zip:

```yaml
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.asset_name }}
          path: |
            ${{ matrix.asset_name }}.tar.gz
            ${{ matrix.asset_name }}.zip
```

- [ ] **Step 5: Ensure the release step handles .zip files**

The existing `find artifacts -type f -name "*.tar.gz"` needs to also include .zip files:

```yaml
      - name: Prepare release files
        run: |
          mkdir -p release_files
          find artifacts -type f \( -name "*.tar.gz" -o -name "*.zip" \) -exec cp {} release_files/ \;
          echo "Files in release_files:"
          ls -la release_files/
```

- [ ] **Step 6: Exclude Windows from Unix-only steps**

Add `if: runner.os != 'Windows'` to:
- "Install cross" step
- "Install native tools" step
- "Configure cargo for native cross-compilation" step
- "Build with cross" step
- "Build with cargo (simple targets)" step — add `&& runner.os != 'Windows'`
- All "Strip binary" steps

Also update the "Create tarball" step: `if: runner.os != 'Windows'`

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add Windows build targets to release workflow

- Add x86_64-pc-windows-msvc and i686-pc-windows-msvc
- Package Windows builds as .zip
- Run tests on Windows in CI

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 8: Update documentation

**Files:**
- Modify: `README.md`
- Modify: `RELEASE.md`

- [ ] **Step 1: Update `README.md`**

Add Windows to the "Supported Platforms" list in the installation section:
```markdown
**Supported Platforms:**
- Linux (x86_64, i686, aarch64, armv7) - both GNU and musl
- macOS (x86_64, Apple Silicon)
- Windows (x86_64, i686)
```

Add a Windows installation section after the Linux/macOS one-liner:

```markdown
#### Windows (PowerShell)

```powershell
irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.ps1 | iex
```

**Uninstall:**
```powershell
irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.ps1 | iex -Args uninstall
```
```

Add a Windows manual installation section:
```markdown
**Windows:**
```powershell
# Extract the downloaded zip and move agm.exe to a directory in your PATH
Expand-Archive agm-windows-*.zip -DestinationPath .
Move-Item agm.exe "$env:USERPROFILE\.local\bin\"
```
```

Add a "Windows Notes" subsection:
```markdown
### Windows Notes

- AGM uses NTFS **junctions** (directories) and **hardlinks** (files) instead of symlinks
- No administrator privileges required
- Central directories and tool config directories must be on the **same drive**
- Default editor fallback: `notepad` (configurable via `editor` in config.toml or `$EDITOR`)
```

- [ ] **Step 2: Update `RELEASE.md`**

Add Windows to the supported platforms section:

```markdown
### Windows
- x86_64 (MSVC)
- i686 (MSVC)
```

Update artifact naming:
```markdown
- Windows: `agm-windows-{arch}.zip`
```

- [ ] **Step 3: Commit**

```bash
git add README.md RELEASE.md
git commit -m "docs: add Windows platform documentation

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

> **Note:** The spec (§5.1) calls for a PowerShell install script (`gpinstall.ps1`). This script is maintained externally as a GitHub Gist (referenced in README). Creating/updating it is out of scope for this plan — handle it as a separate task after the core Windows support lands.

---

### Task 9: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: ALL tests pass

- [ ] **Step 2: Build release binary**

Run: `cargo build --release`
Expected: Successful build with no warnings related to our changes

- [ ] **Step 3: Verify no remaining Unix-specific imports**

Run: `grep -rn "std::os::unix" src/` (or `Select-String`)
Expected: Only appears inside `src/platform.rs` in the `#[cfg(unix)] mod sys` block

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 5: Test `agm init` and `agm status` manually**

Run: `cargo run -- init` (with a test config)
Expected: Initializes successfully, shows capability check on Windows

Run: `cargo run -- status`
Expected: Shows tool status correctly

- [ ] **Step 6: Final commit (if any fixups needed)**

```bash
git add -A
git commit -m "chore: final cleanup for Windows platform support

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```
