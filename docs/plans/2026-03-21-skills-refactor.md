# Skills Architecture Refactor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor AGM's skills management to cleanly separate link/skills concerns, add multi-select installation, grouped listing by source, and an interactive TUI for skill management.

**Architecture:** Incremental refactor — new `src/manage.rs` for TUI (ratatui + crossterm), refactored `skills.rs` with new data model (`SkillInfo`, `SourceGroup`, `SkillInstallStatus`), adjusted `main.rs` routing (`Remove` → hidden alias, `Manage` added), and updated `status.rs` list output. `agm link` is untouched in behavior.

**Tech Stack:** Rust, ratatui 0.29+, crossterm 0.28+, dialoguer (existing), clap (existing)

**Spec:** `docs/specs/skills-refactor.md`

**Baseline:** 50 tests passing. Run `cargo test` to verify.

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `Cargo.toml` | Modify | Add `ratatui`, `crossterm` deps |
| `src/skills.rs` | Refactor | New types + `scan_all_sources()`, `install_skill()`, `uninstall_skill()`, `delete_source()`, `add_local_copy()`, `clone_or_pull()`. Keep existing `scan_skills`, `prune_broken_skills`, `is_url`, `repo_name_from_url`, `find_git_root`. Remove `remove_skill`, `add_local`, `add_from_url`, `list_skills`, `update_all` (replaced by new API). |
| `src/manage.rs` | **New** | TUI app: ratatui rendering, crossterm events, panic hook, editor suspend/resume |
| `src/main.rs` | Modify | CLI enum changes, multi-select flow, manage routing, remove deprecated flow |
| `src/config.rs` | Modify | Add `remove_skill_repo()` method |
| `src/status.rs` | Modify | Use `scan_all_sources()` for grouped `skills list` output |

### Unchanged files
`linker.rs`, `platform.rs`, `files.rs`, `paths.rs`, `editor.rs`, `init.rs`

---

## Task 1: Add Dependencies

**Files:**
- Modify: `Cargo.toml:9-17`

- [ ] **Step 1: Add ratatui and crossterm to Cargo.toml**

In `Cargo.toml`, add after the `dialoguer` line (line 17):

```toml
ratatui = { version = "0.29", default-features = false, features = ["crossterm"] }
crossterm = "0.28"
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: Builds successfully with new deps downloaded

- [ ] **Step 3: Verify existing tests**

Run: `cargo test`
Expected: All 50 tests pass

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add ratatui and crossterm dependencies

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 2: Add `remove_skill_repo()` to Config

**Files:**
- Modify: `src/config.rs:165-173`
- Test: `src/config.rs` (existing test module)

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)]` module in `src/config.rs`:

```rust
#[test]
fn test_remove_skill_repo() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let mut config = Config::default_config();
    config.central.skill_repos = vec![
        "https://github.com/user/repo1.git".to_string(),
        "https://github.com/user/repo2.git".to_string(),
    ];
    config.save_to(&config_path).unwrap();

    config.remove_skill_repo("https://github.com/user/repo1.git");
    assert_eq!(config.central.skill_repos.len(), 1);
    assert_eq!(
        config.central.skill_repos[0],
        "https://github.com/user/repo2.git"
    );

    // Removing non-existent URL is a no-op
    config.remove_skill_repo("https://github.com/user/nonexistent.git");
    assert_eq!(config.central.skill_repos.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_remove_skill_repo`
Expected: FAIL — method `remove_skill_repo` not found

- [ ] **Step 3: Implement `remove_skill_repo`**

Add after `add_skill_repo` method (after line 173) in `src/config.rs`:

```rust
/// Remove a skill repo URL if present
pub fn remove_skill_repo(&mut self, url: &str) {
    self.central.skill_repos.retain(|u| u != url);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_remove_skill_repo`
Expected: PASS

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests pass (50 existing + 1 new = 51)

- [ ] **Step 6: Commit**

```bash
git add src/config.rs
git commit -m "feat: add remove_skill_repo method to Config

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 3: New Data Types in skills.rs

**Files:**
- Modify: `src/skills.rs:1-8` (add types after use statements)

- [ ] **Step 1: Add new type definitions**

After the existing `use` statements (line 7) in `src/skills.rs`, add:

```rust

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
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: Builds (types defined but unused — warnings OK)

- [ ] **Step 3: Verify all tests still pass**

Run: `cargo test`
Expected: All 51 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/skills.rs
git commit -m "feat: add SkillInfo, SourceGroup, SkillInstallStatus types

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 4: Implement `install_skill()` and `uninstall_skill()`

**Files:**
- Modify: `src/skills.rs`

These are the atomic operations that `manage` and `add` will use.

- [ ] **Step 1: Write tests**

Add to the test module in `src/skills.rs`:

```rust
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
    // Second install with different source should fail
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
    // Source should still exist
    assert!(source.exists());
}

#[test]
fn test_uninstall_nonexistent() {
    let dir = tempfile::tempdir().unwrap();
    let skills_dir = dir.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();
    // Should not error — just a no-op
    let result = uninstall_skill("nonexistent", &skills_dir);
    assert!(result.is_ok());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_install_skill test_uninstall`
Expected: FAIL — functions not found

- [ ] **Step 3: Implement functions**

Add after `remove_skill` function (around line 143) in `src/skills.rs`:

```rust
/// Install a single skill by creating a symlink in the central skills directory.
/// Errors if a skill with the same name from a different source already exists.
pub fn install_skill(name: &str, source_path: &Path, skills_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(skills_dir)?;
    let link_path = skills_dir.join(name);

    if link_path.exists() || link_path.symlink_metadata().is_ok() {
        // Check if it already points to the same source
        if platform::is_dir_link(&link_path) {
            if let Some(target) = platform::read_dir_link_target(&link_path) {
                let target_canon = fs::canonicalize(&target).unwrap_or(target);
                let source_canon = fs::canonicalize(source_path).unwrap_or(source_path.to_path_buf());
                if target_canon == source_canon {
                    return Ok(()); // Already installed from same source
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
        return Ok(()); // Not installed, nothing to do
    }
    if platform::is_dir_link(&link_path) {
        platform::remove_link(&link_path)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test test_install_skill test_uninstall`
Expected: All 4 new tests PASS

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add src/skills.rs
git commit -m "feat: add install_skill and uninstall_skill functions

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 5: Implement `scan_all_sources()`

**Files:**
- Modify: `src/skills.rs`

This is the core scanning function that `list` and `manage` both use.

- [ ] **Step 1: Write tests**

Add to the test module in `src/skills.rs`:

```rust
#[test]
fn test_scan_all_sources_empty() {
    let dir = tempfile::tempdir().unwrap();
    let source_dir = dir.path().join("source");
    let skills_dir = dir.path().join("skills");
    // Neither directory exists
    let groups = scan_all_sources(&source_dir, &skills_dir, &[]);
    assert!(groups.is_empty());
}

#[test]
fn test_scan_all_sources_repo() {
    let dir = tempfile::tempdir().unwrap();
    let source_dir = dir.path().join("source");
    let skills_dir = dir.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    // Create a fake repo with 2 skills
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
    // Both should be NotInstalled
    assert!(groups[0].skills.iter().all(|s| s.install_status == SkillInstallStatus::NotInstalled));
}

#[test]
fn test_scan_all_sources_local() {
    let dir = tempfile::tempdir().unwrap();
    let source_dir = dir.path().join("source");
    let skills_dir = dir.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    // Create a local source
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

    // Create a migrated source
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

    // Create repo with one skill
    let repo = source_dir.join("my-repo");
    let skill_path = repo.join("cool-skill");
    fs::create_dir_all(&skill_path).unwrap();
    fs::write(skill_path.join("SKILL.md"), "# Cool").unwrap();

    // Install it
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

    // Create two repos each with a "common-skill"
    let repo_a = source_dir.join("repo-a");
    let skill_a = repo_a.join("common-skill");
    fs::create_dir_all(&skill_a).unwrap();
    fs::write(skill_a.join("SKILL.md"), "# A").unwrap();

    let repo_b = source_dir.join("repo-b");
    let skill_b = repo_b.join("common-skill");
    fs::create_dir_all(&skill_b).unwrap();
    fs::write(skill_b.join("SKILL.md"), "# B").unwrap();

    // Install from repo-a
    install_skill("common-skill", &skill_a, &skills_dir).unwrap();

    let groups = scan_all_sources(&source_dir, &skills_dir, &[]);
    // repo-a's skill should be Installed, repo-b's should be Conflict
    let group_a = groups.iter().find(|g| g.name == "repo-a").unwrap();
    let group_b = groups.iter().find(|g| g.name == "repo-b").unwrap();
    assert_eq!(group_a.skills[0].install_status, SkillInstallStatus::Installed);
    assert_eq!(group_b.skills[0].install_status, SkillInstallStatus::Conflict);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_scan_all_sources`
Expected: FAIL — function `scan_all_sources` not found

- [ ] **Step 3: Implement `resolve_repo_url` helper**

Add a helper function in `src/skills.rs` (private):

```rust
/// Try to resolve the git remote URL for a directory.
/// First checks config skill_repos, then falls back to git remote.
fn resolve_repo_url(dir_name: &str, path: &Path, skill_repos: &[String]) -> Option<String> {
    // Try matching against config skill_repos
    for url in skill_repos {
        if repo_name_from_url(url) == dir_name {
            return Some(url.clone());
        }
    }
    // Fallback: git remote get-url origin
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
```

- [ ] **Step 4: Implement `check_install_status` helper**

```rust
/// Check the install status of a skill by examining the central skills directory.
fn check_install_status(name: &str, source_path: &Path, skills_dir: &Path) -> SkillInstallStatus {
    let link_path = skills_dir.join(name);
    if !link_path.symlink_metadata().is_ok() {
        return SkillInstallStatus::NotInstalled;
    }
    if platform::is_dir_link(&link_path) {
        if let Some(target) = platform::read_dir_link_target(&link_path) {
            let target_canon = std::fs::canonicalize(&target).unwrap_or(target);
            let source_canon = std::fs::canonicalize(source_path).unwrap_or(source_path.to_path_buf());
            if target_canon == source_canon {
                return SkillInstallStatus::Installed;
            }
        }
    }
    SkillInstallStatus::Conflict
}
```

- [ ] **Step 5: Implement `scan_all_sources`**

```rust
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
            // Each subdirectory under local/ is a Local source
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
            // Each subdirectory under agm_tools/ is a Migrated source
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
            // Everything else is a Repo source
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
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test test_scan_all_sources`
Expected: All 6 new tests PASS

- [ ] **Step 7: Run all tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 8: Commit**

```bash
git add src/skills.rs
git commit -m "feat: implement scan_all_sources with source grouping and install status

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 6: Implement `add_local_copy()` and `clone_or_pull()`

**Files:**
- Modify: `src/skills.rs`

These replace `add_local` and `add_from_url` — they separate clone/copy from install, returning the skill list for the caller to handle multi-select.

- [ ] **Step 1: Write tests**

Add to test module in `src/skills.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_add_local_copy`
Expected: FAIL — function not found

- [ ] **Step 3: Implement `add_local_copy`**

Add in `src/skills.rs`:

```rust
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
```

- [ ] **Step 4: Implement `clone_or_pull`**

Add in `src/skills.rs`:

```rust
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
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test test_add_local_copy`
Expected: All 3 new tests PASS

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add src/skills.rs
git commit -m "feat: add add_local_copy and clone_or_pull (split clone from install)

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 7: Implement `delete_source()`

**Files:**
- Modify: `src/skills.rs`

- [ ] **Step 1: Write test**

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_delete_source`
Expected: FAIL — function not found

- [ ] **Step 3: Implement**

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_delete_source`
Expected: PASS

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add src/skills.rs
git commit -m "feat: add delete_source function

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 8: Refactor `update_all()` to Use New API

**Files:**
- Modify: `src/skills.rs`

The existing `update_all` auto-installs new skills found after pull. Refactor it to use `install_skill()` (consistent with new API) and ensure prune behavior matches spec (prune broken links only, don't auto-rebuild).

- [ ] **Step 1: Refactor `update_all`**

Replace the existing `update_all` function body (lines 225-298) with:

```rust
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
            if !link_path.symlink_metadata().is_ok() {
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
```

Note: The function signature changes to take `source_dir` as parameter. The caller in main.rs already has `source_dir` available.

- [ ] **Step 2: Update the `SkillsAction::Update` call site in main.rs**

In `src/main.rs`, find the `SkillsAction::Update` match arm (around line 796) and update:

```rust
SkillsAction::Update => {
    skills::update_all(&skills_dir, &source_dir)?;
    Ok(())
}
```

- [ ] **Step 3: Update the `skill_repos` processing during `agm link` in main.rs**

In `src/main.rs`, find the `skill_repos` block (lines 434-448) and replace:

```rust
// Process skill_repos when linking skills
if !config.central.skill_repos.is_empty() {
    println!("\n{}", "Processing skill repositories...".bold());
    for url in &config.central.skill_repos {
        match skills::add_from_url(url, &source_dir, &central_skills) {
```

With code using the new `clone_or_pull` + `install_skill` API:

```rust
// Process skill_repos when linking skills
if !config.central.skill_repos.is_empty() {
    println!("\n{}", "Processing skill repositories...".bold());
    for url in &config.central.skill_repos {
        match skills::clone_or_pull(url, &source_dir) {
            Ok((_repo_path, found_skills)) => {
                let mut count = 0;
                for (name, skill_path) in found_skills {
                    if let Ok(()) = skills::install_skill(&name, &skill_path, &central_skills) {
                        count += 1;
                    }
                }
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
```

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/skills.rs src/main.rs
git commit -m "refactor: update_all uses source_dir scanning and install_skill

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 9: Update `main.rs` CLI — SkillsAction Enum + Add Multi-Select

**Files:**
- Modify: `src/main.rs:66-76` (SkillsAction enum)
- Modify: `src/main.rs:720-799` (Commands::Skills match arm)

- [ ] **Step 1: Update SkillsAction enum**

Replace the `SkillsAction` enum (lines 66-76) in `src/main.rs`:

```rust
#[derive(Subcommand)]
enum SkillsAction {
    /// List all skills grouped by source
    List,
    /// Install skill(s) from local path or repo URL
    Add {
        source: String,
        /// Install all skills without prompting
        #[arg(short = 'a', long = "all")]
        all: bool,
    },
    /// Interactive skill manager (TUI)
    Manage {
        /// Source name to manage, or "all" for all sources
        name: Option<String>,
    },
    /// Git pull all skill source repos
    Update,
    /// (deprecated, use 'manage' instead)
    #[command(hide = true)]
    Remove {
        name: String,
    },
}
```

- [ ] **Step 2: Update the Commands::Skills match arm**

Replace the entire `Commands::Skills` handler (lines 720-799) in `src/main.rs`. Key changes:
- Interactive menu: replace "remove" with "manage"
- Add: multi-select with `--all` flag
- Manage: route to `manage::run()`
- Remove: print deprecation message
- List: use `scan_all_sources()` for grouped output

```rust
Commands::Skills { action } => {
    let mut config = config::Config::load_from(cli.config.clone())?;
    let skills_dir = paths::expand_tilde(&config.central.skills_source);
    let source_dir = paths::expand_tilde(&config.central.source_dir);

    let action = match action {
        Some(a) => a,
        None => {
            use dialoguer::{theme::ColorfulTheme, Select};
            let labels = [
                "list        show all skills grouped by source",
                "add         install skill(s) from path or URL",
                "manage      interactive skill manager",
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
                    SkillsAction::Add { source, all: false }
                }
                2 => SkillsAction::Manage { name: None },
                _ => SkillsAction::Update,
            }
        }
    };

    match action {
        SkillsAction::List => {
            let pruned = skills::prune_broken_skills(&skills_dir)?;
            if pruned > 0 {
                println!("  {} Removed {} broken link(s)", "warn".yellow(), pruned);
            }
            let groups = skills::scan_all_sources(
                &source_dir,
                &skills_dir,
                &config.central.skill_repos,
            );
            if groups.is_empty() {
                println!("No skill sources found. Use 'agm skills add' to add a source.");
            } else {
                println!();
                let mut total = 0;
                let mut installed = 0;
                for group in &groups {
                    let icon = match &group.kind {
                        skills::SourceKind::Repo { .. } => "📦",
                        skills::SourceKind::Local => "📁",
                        skills::SourceKind::Migrated { .. } => "📁",
                    };
                    let detail = match &group.kind {
                        skills::SourceKind::Repo { url } => {
                            url.as_deref().map(|u| format!("repo: {}", u)).unwrap_or_else(|| "repo".into())
                        }
                        skills::SourceKind::Local => "local".into(),
                        skills::SourceKind::Migrated { tool } => format!("migrated from {}", tool),
                    };
                    println!("{} {} ({})", icon, group.name.bold(), detail);
                    for skill in &group.skills {
                        total += 1;
                        let (indicator, status_text) = match skill.install_status {
                            skills::SkillInstallStatus::Installed => {
                                installed += 1;
                                ("✓".green().to_string(), "installed".green().to_string())
                            }
                            skills::SkillInstallStatus::NotInstalled => {
                                ("✗".dimmed().to_string(), "not installed".dimmed().to_string())
                            }
                            skills::SkillInstallStatus::Conflict => {
                                ("⚡".yellow().to_string(), "conflict".yellow().to_string())
                            }
                        };
                        println!("   {} {:<24} {}", indicator, skill.name, status_text);
                    }
                    println!();
                }
                println!(
                    "── {} ──",
                    format!(
                        "{} source(s), {} skill(s) ({} installed, {} not installed)",
                        groups.len(),
                        total,
                        installed,
                        total - installed
                    )
                    .bold()
                );
            }
            Ok(())
        }
        SkillsAction::Add { source, all } => {
            if skills::is_url(&source) {
                let (repo_path, found_skills) =
                    skills::clone_or_pull(&source, &source_dir)?;
                config.add_skill_repo(&source)?;
                let to_install = select_skills_to_install(&found_skills, all)?;
                let mut count = 0;
                for (name, skill_path) in to_install {
                    match skills::install_skill(&name, &skill_path, &skills_dir) {
                        Ok(()) => {
                            println!(
                                "  {} {} → {}",
                                " ok ".green(),
                                name,
                                paths::contract_tilde(&skill_path)
                            );
                            count += 1;
                        }
                        Err(e) => println!("  {} {}: {}", "warn".yellow(), name, e),
                    }
                }
                println!("\n{} skill(s) installed from {}.", count, paths::contract_tilde(&repo_path));
            } else {
                let source_path = paths::expand_tilde(&source);
                println!(
                    "Adding skills from {}...",
                    paths::contract_tilde(&source_path)
                );
                let (dest, found_skills) =
                    skills::add_local_copy(&source_path, &source_dir)?;
                let to_install = select_skills_to_install(&found_skills, all)?;
                let mut count = 0;
                for (name, skill_path) in to_install {
                    match skills::install_skill(&name, &skill_path, &skills_dir) {
                        Ok(()) => {
                            println!(
                                "  {} {} → {}",
                                " ok ".green(),
                                name,
                                paths::contract_tilde(&skill_path)
                            );
                            count += 1;
                        }
                        Err(e) => println!("  {} {}: {}", "warn".yellow(), name, e),
                    }
                }
                println!("\n{} skill(s) installed from {}.", count, paths::contract_tilde(&dest));
            }
            Ok(())
        }
        SkillsAction::Manage { name } => {
            manage::run(&mut config, name.as_deref())?;
            Ok(())
        }
        SkillsAction::Remove { name: _ } => {
            println!(
                "{}\n{}",
                "'agm skills remove' has been replaced by 'agm skills manage'.".yellow(),
                "Use 'agm skills manage' to interactively install/uninstall skills."
            );
            Ok(())
        }
        SkillsAction::Update => {
            skills::update_all(&skills_dir, &source_dir)?;
            Ok(())
        }
    }
}
```

- [ ] **Step 3: Add `select_skills_to_install` helper function**

Add this helper in `src/main.rs` (near the other helper functions, after `pick_target`):

```rust
/// If there is only 1 skill, return it directly. If multiple and `all` is true, return all.
/// Otherwise show a MultiSelect dialog and return the selected skills.
fn select_skills_to_install(
    skills: &[(String, PathBuf)],
    all: bool,
) -> anyhow::Result<Vec<(String, PathBuf)>> {
    if skills.len() <= 1 || all {
        return Ok(skills.to_vec());
    }

    use dialoguer::{theme::ColorfulTheme, MultiSelect};

    let labels: Vec<&str> = skills.iter().map(|(name, _)| name.as_str()).collect();

    let defaults: Vec<bool> = vec![true; skills.len()];

    let selected = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Select skills to install ({} found)", skills.len()))
        .items(&labels)
        .defaults(&defaults)
        .interact()?;

    Ok(selected.into_iter().map(|i| skills[i].clone()).collect())
}
```

- [ ] **Step 4: Add `mod manage;` declaration**

Add `mod manage;` after line 8 (`mod skills;`) in `src/main.rs`:

```rust
mod manage;
```

- [ ] **Step 5: Create stub `src/manage.rs`**

Create `src/manage.rs` with a stub implementation so the build succeeds:

```rust
use crate::config::Config;

/// Interactive TUI for managing skills (stub — will be implemented in Task 11)
pub fn run(_config: &mut Config, _source_filter: Option<&str>) -> anyhow::Result<()> {
    println!("Skills manager TUI is not yet implemented.");
    Ok(())
}
```

- [ ] **Step 6: Verify build**

Run: `cargo build`
Expected: Builds successfully

- [ ] **Step 7: Run all tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 8: Commit**

```bash
git add src/main.rs src/manage.rs
git commit -m "feat: update CLI — add multi-select, manage subcommand, deprecate remove

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 10: Remove Old Functions from skills.rs

**Files:**
- Modify: `src/skills.rs`
- Modify: `src/skills.rs` (test module)

Now that the new API is in place and main.rs uses it, remove the old functions that are no longer called.

- [ ] **Step 1: Remove `add_local`, `add_from_url`, `remove_skill`, `list_skills`**

Delete these function bodies from `src/skills.rs`:
- `add_local` (lines 85-119)
- `remove_skill` (lines 122-143)
- `add_from_url` (lines 188-222)
- `list_skills` (lines 58-82)
- `find_git_root` (lines 301-313) — no longer called after `update_all` refactor

Keep: `scan_skills`, `scan_skills_recursive`, `prune_broken_skills`, `is_url`, `repo_name_from_url`, `update_all`, and all new functions.

- [ ] **Step 2: Remove old tests that reference deleted functions**

Remove tests:
- `test_add_local_single` (references old `add_local`)
- `test_add_local_idempotent` (references old `add_local`)
- `test_remove_skill` (references old `remove_skill`)
- `test_list_skills` (references old `list_skills`)

Keep: `test_scan_skills_single`, `test_scan_skills_multiple`, `test_prune_broken_skills`, and all new tests.

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: No errors, no warnings about unused functions

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All remaining tests pass

- [ ] **Step 5: Commit**

```bash
git add src/skills.rs
git commit -m "refactor: remove old add_local, add_from_url, remove_skill, list_skills

Replaced by add_local_copy, clone_or_pull, install_skill, uninstall_skill,
scan_all_sources.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 11: Implement `manage.rs` — TUI Core

**Files:**
- Modify: `src/manage.rs` (replace stub)

This is the largest task. The TUI has these components:
1. App state struct
2. Terminal setup/teardown with panic hook
3. Event loop (crossterm events)
4. Rendering (ratatui widgets)
5. Actions (toggle, delete, editor, search, info)

- [ ] **Step 1: Implement the full `manage.rs` module**

Replace the stub `src/manage.rs` with the complete implementation. The file structure:

```rust
use std::io::{self, stdout};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Context;
use colored::Colorize;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::config::Config;
use crate::editor;
use crate::paths::{self, contract_tilde, expand_tilde};
use crate::skills::{
    self, SkillInstallStatus, SkillInfo, SourceGroup, SourceKind,
    delete_source, install_skill, prune_broken_skills, scan_all_sources, uninstall_skill,
};

/// A row in the TUI list — either a source header or a skill entry
#[derive(Debug, Clone)]
enum ListRow {
    SourceHeader { group_index: usize },
    Skill { group_index: usize, skill_index: usize },
}

/// Application state
struct App {
    config: Config,
    groups: Vec<SourceGroup>,
    rows: Vec<ListRow>,
    cursor: usize,
    scroll_offset: usize,
    status_message: Option<(String, Instant)>,
    search_mode: bool,
    search_query: String,
    filtered_rows: Option<Vec<usize>>, // indices into `rows`
    should_quit: bool,
    source_filter: Option<String>,
}

// ... (App impl methods for: rebuild_rows, visible_rows, toggle_skill,
//      toggle_source, delete_current_source, refresh, etc.)
```

The implementation is large (~500 lines). Key sections:

**`App::new()`** — Load groups via `scan_all_sources()`, build row list.

**`App::rebuild_rows()`** — Flatten groups into `Vec<ListRow>` for display. If `filtered_rows` is active, only include matching skills and their source headers.

**`App::toggle_skill()`** — Call `install_skill` or `uninstall_skill`, update group in place, set status message.

**`App::toggle_source()`** — Toggle all skills in a source group.

**`App::delete_current_source()`** — For `ListRow::SourceHeader`, confirm deletion inline then call `delete_source()`, remove URL from config if repo, refresh.

**`App::open_editor()`** — Get SKILL.md path, suspend terminal, launch editor, resume terminal.

**`App::apply_search()`** — Case-insensitive filter on skill names, rebuild `filtered_rows`.

**`pub fn run()`** — Entry point:
1. Check for zero sources (exit early with message)
2. If no `source_filter` and multiple sources, show dialoguer Select
3. Install panic hook for terminal restoration
4. Enter alternate screen + raw mode
5. Create ratatui Terminal
6. Run event loop
7. Restore terminal on exit

**Rendering** — Uses ratatui `List` widget with custom styling:
- Source headers: bold with icon (📦/📁) and skill count
- Skills: indented with ✓/✗/⚡ indicator
- Bottom bar: keybinding help
- Status bar: messages, search input, info display

**Event Loop:**
```rust
loop {
    terminal.draw(|f| app.render(f))?;
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press { continue; }
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Up => app.move_cursor(-1),
                KeyCode::Down => app.move_cursor(1),
                KeyCode::PageUp => app.move_cursor(-(visible_height as i32)),
                KeyCode::PageDown => app.move_cursor(visible_height as i32),
                KeyCode::Char(' ') => app.toggle_current()?,
                KeyCode::Char('e') => app.open_editor(&mut terminal)?,
                KeyCode::Delete | KeyCode::Char('d') => app.delete_current_source()?,
                KeyCode::Char('i') => app.show_info(),
                KeyCode::Char('r') => app.refresh()?,
                KeyCode::Char('/') => app.enter_search_mode(),
                KeyCode::Esc => app.clear_search(),
                _ if app.search_mode => app.handle_search_input(key),
                _ => {}
            }
        }
    }
    // Clear expired status messages
    app.clear_expired_status();
}
```

**Editor suspend/resume:**
```rust
fn open_editor(&mut self, terminal: &mut Terminal<impl Backend>) -> anyhow::Result<()> {
    // Get SKILL.md path for current skill
    let skill_md = match self.current_row() {
        Some(ListRow::Skill { group_index, skill_index }) => {
            self.groups[*group_index].skills[*skill_index]
                .source_path.join("SKILL.md")
        }
        _ => return Ok(()),
    };

    // Suspend TUI
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    // Launch editor
    let ed = editor::get_editor(&self.config);
    let _ = editor::open_files(&ed, &[skill_md.as_path()]);

    // Resume TUI
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;

    Ok(())
}
```

**Delete confirmation (inline in TUI):**
```rust
fn delete_current_source(&mut self, terminal: &mut Terminal<impl Backend>) -> anyhow::Result<()> {
    let group_index = match self.current_row() {
        Some(ListRow::SourceHeader { group_index }) => *group_index,
        _ => return Ok(()),
    };
    let group = &self.groups[group_index];
    let n_skills = group.skills.len();

    let is_migrated = matches!(group.kind, SourceKind::Migrated { .. });
    // Set status message as prompt, then wait for y/n keypress
    if is_migrated {
        self.status_message = Some((
            format!(
                "⚠ WARNING: Migrated skills are UNRECOVERABLE. Type 'delete' to confirm deleting \"{}\" ({} skills): ",
                group.name, n_skills
            ),
            Instant::now() + Duration::from_secs(300),
        ));
        // ... collect typed chars, match "delete"
    } else {
        self.status_message = Some((
            format!("Delete \"{}\" and {} skill(s)? [y/N] ", group.name, n_skills),
            Instant::now() + Duration::from_secs(300),
        ));
        // ... wait for y/n keypress
    }
    // On confirm: delete_source(), remove_skill_repo if repo, refresh
}
```

Implementation note: Due to the complexity, write this file incrementally — start with the basic skeleton (App struct, terminal setup, simple rendering, cursor movement) then add features one by one. Verify each addition compiles.

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: Builds successfully

- [ ] **Step 3: Manual test**

Run: `cargo run -- skills manage`
Expected: TUI renders (may show "No skill sources found" if no sources configured)

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests still pass

- [ ] **Step 5: Commit**

```bash
git add src/manage.rs
git commit -m "feat: implement interactive TUI skill manager (ratatui + crossterm)

Supports: cursor navigation, space toggle install/uninstall, e to edit
SKILL.md, Del to delete source, / search, r refresh, i info, q quit.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 12: Update `status.rs` for Grouped List

**Files:**
- Modify: `src/status.rs`

The `skills list` command now uses `scan_all_sources()` in `main.rs` directly (Task 9). But `status.rs` also shows skill count in the status display. Update it to use the new scanning.

- [ ] **Step 1: Update `status.rs` skill count display**

First, add `use crate::skills;` to the imports at the top of `src/status.rs` (after line 7):

```rust
use crate::skills;
```

Then replace the skill count section (lines 138-153) with grouped counting:

```rust
// Count skills from all sources
let groups = skills::scan_all_sources(
    &expand_tilde(&config.central.source_dir),
    &central_skills,
    &config.central.skill_repos,
);
let total_skills: usize = groups.iter().map(|g| g.skills.len()).sum();
let installed_skills: usize = groups.iter()
    .flat_map(|g| &g.skills)
    .filter(|s| s.install_status == skills::SkillInstallStatus::Installed)
    .count();

println!("Central prompt : {}", contract_tilde(&central_prompt));
println!(
    "Central skills : {} ({} installed, {} sources)",
    contract_tilde(&central_skills),
    installed_skills,
    groups.len()
);
let source_dir = expand_tilde(&config.central.source_dir);
println!("Central source : {}", contract_tilde(&source_dir));
println!("Central files  : {}", contract_tilde(&files_base));
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: Builds

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/status.rs
git commit -m "feat: status display shows skill install count from scan_all_sources

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 13: Final Verification and Cleanup

**Files:**
- All modified files

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Check for unused imports/dead code**

Run: `cargo build 2>&1 | grep warning`
Expected: No warnings (or only expected ones)

- [ ] **Step 4: Verify release build**

Run: `cargo build --release`
Expected: Builds successfully

- [ ] **Step 5: Manual smoke test**

Test these commands work:
```bash
cargo run -- skills list
cargo run -- skills manage
cargo run -- skills remove test    # Should show deprecation message
cargo run -- skills add --help     # Should show --all/-a flag
```

- [ ] **Step 6: Final commit (if any cleanup needed)**

```bash
git add -A
git commit -m "chore: final cleanup for skills architecture refactor

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```
