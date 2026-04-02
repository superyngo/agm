# Commands Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add commands support (custom slash-command prompt templates as `.md` files in `commands/` folders) to AGM, mirroring the existing agents pattern across config, data layer, TUI, and status.

**Architecture:** Commands follow the same architecture as agents — `.md` files discovered from source repo `commands/` subdirectories, installed as file symlinks into a central commands directory, and linked into each tool's config directory as a directory symlink. All existing agents functions are replicated for commands.

**Tech Stack:** Rust, ratatui (TUI), serde/toml (config), tempfile (tests)

---

### Task 1: Config Layer — Add commands_source and commands_dir

**Files:**
- Modify: `src/config.rs:17-31` (CentralConfig struct + impl)
- Modify: `src/config.rs:33-49` (ToolConfig struct)
- Modify: `src/config.rs:85-197` (default_config)

- [ ] **Step 1: Add `commands_source` field to `CentralConfig`**

In `src/config.rs`, after the `agents_source` field (line 21), add the `commands_source` field:

```rust
// In CentralConfig struct, after agents_source:
    #[serde(default = "CentralConfig::default_commands_source")]
    pub commands_source: String,
```

In the `impl CentralConfig` block (after `default_agents_source`, line 28-30), add:

```rust
    fn default_commands_source() -> String {
        "~/.local/share/agm/commands".into()
    }
```

- [ ] **Step 2: Add `commands_dir` field to `ToolConfig`**

In `src/config.rs`, after the `agents_dir` field (line 46), add:

```rust
    #[serde(default)]
    pub commands_dir: String,
```

- [ ] **Step 3: Update `default_config()` — every tool gets `commands_dir`**

In `src/config.rs` `default_config()`, add `commands_dir: "commands".into(),` to each tool's `ToolConfig`. Add it after `agents_dir: "agents".into(),` for each of these tools:
- claude (after line 100)
- codex (after line 113)
- copilot (after line 126)
- crush (after line 139)
- gemini (after line 156)
- opencode (after line 169)
- pi (after line 182)

Also update the `CentralConfig` initializer (lines 189-195). After `agents_source`:

```rust
                agents_source: "~/.local/share/agm/agents".into(),
                commands_source: "~/.local/share/agm/commands".into(),
```

- [ ] **Step 4: Build to verify no compile errors**

Run: `cargo build 2>&1 | head -20`
Expected: Build succeeds (or only warnings).

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: add commands_source and commands_dir to config"
```

---

### Task 2: Init Layer — Create commands directory on `agm init`

**Files:**
- Modify: `src/init.rs:34-38` (dirs_to_create array)

- [ ] **Step 1: Add commands_source to dirs_to_create**

In `src/init.rs`, add `&config.central.commands_source` to the `dirs_to_create` array (after line 36):

```rust
    let dirs_to_create = [
        &config.central.skills_source,
        &config.central.agents_source,
        &config.central.commands_source,
        &config.central.source_dir,
    ];
```

- [ ] **Step 2: Build to verify**

Run: `cargo build 2>&1 | head -20`
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add src/init.rs
git commit -m "feat: create commands directory on agm init"
```

---

### Task 3: Data Layer — CommandInfo struct, scan, install, uninstall, prune, check status

**Files:**
- Modify: `src/skills.rs:28-55` (add CommandInfo, extend SourceGroup)
- Modify: `src/skills.rs` (add new functions after agents functions)
- Modify: `src/skills.rs:59-74` (UpdateProgress)
- Modify: `src/skills.rs:624-753` (scan_all_sources)
- Modify: `src/skills.rs:864-889` (delete_source)
- Modify: `src/skills.rs:545-578` (update_all_with_progress)

- [ ] **Step 1: Add `CommandInfo` struct**

In `src/skills.rs`, after `AgentInfo` (line 34), add:

```rust
/// Full info about a single command (.md file in commands/ folder)
#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub name: String,
    pub source_path: PathBuf,
    pub install_status: SkillInstallStatus,
}
```

- [ ] **Step 2: Add `commands` field to `SourceGroup`**

In `src/skills.rs`, in the `SourceGroup` struct (line 54), add after `agents`:

```rust
    pub commands: Vec<CommandInfo>,
```

- [ ] **Step 3: Add `new_commands` to `UpdateProgress::AllDone`**

In `src/skills.rs`, in the `UpdateProgress` enum `AllDone` variant (line 68-73), add:

```rust
    AllDone {
        total: usize,
        updated: usize,
        new_skills: usize,
        new_agents: usize,
        new_commands: usize,
    },
```

- [ ] **Step 4: Add `scan_commands` function**

In `src/skills.rs`, after `scan_agents` (line 120), add:

```rust
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
```

- [ ] **Step 5: Add `install_command` function**

After `install_agent` (line 216), add:

```rust
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
```

- [ ] **Step 6: Add `uninstall_command` function**

After `uninstall_agent` (line 227), add:

```rust
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
```

- [ ] **Step 7: Add `prune_broken_commands` function**

After `prune_broken_agents` (line 281), add:

```rust
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
```

- [ ] **Step 8: Add `check_command_install_status` function**

After `check_agent_install_status` (line 302), add:

```rust
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
```

- [ ] **Step 9: Update `scan_all_sources` signature and body**

Change the function signature (line 624-628) to accept `commands_dir`:

```rust
pub fn scan_all_sources(
    source_dir: &Path,
    skills_dir: &Path,
    agents_dir: &Path,
    commands_dir: &Path,
    source_repos: &[String],
) -> Vec<SourceGroup> {
```

For each of the three source types (local at ~670, migrated at ~706, git repo at ~733), add commands scanning after agents scanning. The pattern for each is identical:

```rust
                    let commands = scan_commands(&sub_path)
                        .into_iter()
                        .map(|(name, sp)| CommandInfo {
                            install_status: check_command_install_status(&name, &sp, commands_dir),
                            name,
                            source_path: sp,
                        })
                        .collect();
```

(For git repos, use `&path` instead of `&sub_path`.)

And add `commands` to each `SourceGroup { ... }` initializer.

- [ ] **Step 10: Update `delete_source` to handle commands**

In `src/skills.rs`, update `delete_source` (line 864-889) to also accept `commands_dir` and uninstall commands:

Change signature:
```rust
pub fn delete_source(
    group: &SourceGroup,
    skills_dir: &Path,
    agents_dir: &Path,
    commands_dir: &Path,
) -> anyhow::Result<()> {
```

Add after the agents uninstall loop (after line 881):
```rust
    // Remove all central symlinks for this source's commands
    for command in &group.commands {
        if command.install_status == SkillInstallStatus::Installed {
            uninstall_command(&command.name, commands_dir)?;
        }
    }
```

- [ ] **Step 11: Update `update_all_with_progress` to handle commands**

In `src/skills.rs`, update `update_all_with_progress` (around lines 456-578):

1. Change signature to accept `commands_dir: &Path` parameter.
2. Add `let _ = prune_broken_commands(commands_dir);` after line 547.
3. Add commands re-sync after agents re-sync (after line 570):

```rust
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
```

4. Update `AllDone` emission to include `new_commands: new_commands_total`.
5. Initialize `let mut new_commands_total = 0usize;` alongside `new_agents_total`.

- [ ] **Step 12: Build to verify**

Run: `cargo build 2>&1 | head -40`
Expected: Will show errors from callers of changed signatures — that's expected, we'll fix them in subsequent tasks.

- [ ] **Step 13: Commit**

```bash
git add src/skills.rs
git commit -m "feat: add CommandInfo, scan/install/uninstall/prune/migrate for commands"
```

---

### Task 4: Data Layer — Add `migrate_commands_dir_quiet`

**Files:**
- Modify: `src/skills.rs` (add after `migrate_agents_dir_quiet`, line 1064)

- [ ] **Step 1: Add `migrate_commands_dir_quiet` function**

After `migrate_agents_dir_quiet` (line 1064), add:

```rust
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
```

- [ ] **Step 2: Commit**

```bash
git add src/skills.rs
git commit -m "feat: add migrate_commands_dir_quiet"
```

---

### Task 5: Background Task — Add `new_commands` to UpdateAllDone

**Files:**
- Modify: `src/tui/background.rs:16-21` (TaskEvent::UpdateAllDone)
- Modify: `src/tui/background.rs:100-144` (spawn_update)
- Modify: `src/tui/background.rs:209-213` (test)

- [ ] **Step 1: Add `new_commands` to `TaskEvent::UpdateAllDone`**

In `src/tui/background.rs`, update the `UpdateAllDone` variant (line 16-21):

```rust
    UpdateAllDone {
        total: usize,
        updated: usize,
        new_skills: usize,
        new_agents: usize,
        new_commands: usize,
    },
```

- [ ] **Step 2: Update `spawn_update` function**

In `src/tui/background.rs`, update `spawn_update` (line 102-144):

1. Add `commands_dir: PathBuf` parameter (after `agents_dir`).
2. Pass `&commands_dir` to `update_all_with_progress`.
3. Add `new_commands` to the `AllDone` → `UpdateAllDone` mapping:

```rust
pub fn spawn_update(
    skills_dir: PathBuf,
    agents_dir: PathBuf,
    commands_dir: PathBuf,
    source_dir: PathBuf,
) -> BackgroundTask {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        crate::skills::update_all_with_progress(
            &skills_dir,
            &agents_dir,
            &commands_dir,
            &source_dir,
            |progress| {
                let event = match progress {
                    crate::skills::UpdateProgress::RepoStart { name } => {
                        TaskEvent::UpdateRepoStart { name }
                    }
                    crate::skills::UpdateProgress::RepoComplete {
                        name,
                        success,
                        message,
                    } => TaskEvent::UpdateRepoComplete {
                        name,
                        success,
                        message,
                    },
                    crate::skills::UpdateProgress::AllDone {
                        total,
                        updated,
                        new_skills,
                        new_agents,
                        new_commands,
                    } => TaskEvent::UpdateAllDone {
                        total,
                        updated,
                        new_skills,
                        new_agents,
                        new_commands,
                    },
                };
                let _ = tx.send(event);
            },
        );
    });
    BackgroundTask::new(rx)
}
```

- [ ] **Step 3: Update test**

In the test at line 209-213, add `new_commands: 0`:

```rust
        tx.send(TaskEvent::UpdateAllDone {
            total: 5,
            updated: 3,
            new_skills: 2,
            new_agents: 1,
            new_commands: 0,
        })
```

- [ ] **Step 4: Build to verify**

Run: `cargo build 2>&1 | head -40`

- [ ] **Step 5: Commit**

```bash
git add src/tui/background.rs
git commit -m "feat: add new_commands to background task events"
```

---

### Task 6: Status Command — Display commands link status

**Files:**
- Modify: `src/status.rs:1-169`

- [ ] **Step 1: Add commands link check and display**

In `src/status.rs`:

1. After `central_agents` (line 12), add:
```rust
    let central_commands = expand_tilde(&config.central.commands_source);
```

2. After the `agents_ls` check (lines 42-47), add:
```rust
        let commands_ls = if !tool.commands_dir.is_empty() {
            let link = tool.resolved_config_dir().join(&tool.commands_dir);
            Some(check_link(&link, &central_commands, true))
        } else {
            None
        };
```

3. After the agents display block (after line 129), add:
```rust
        // Detail lines: commands
        if let Some(ls) = commands_ls {
            let commands_link = tool.resolved_config_dir().join(&tool.commands_dir);
            print!("{}{:<8}", INDENT, "commands");
            match ls {
                LinkStatus::Linked => println!(
                    "{} → {}",
                    "✓ linked".green(),
                    contract_tilde(&commands_link).dimmed()
                ),
                LinkStatus::Missing => println!(
                    "{} → {}",
                    "✗ missing".yellow(),
                    contract_tilde(&central_commands).dimmed()
                ),
                LinkStatus::Broken => println!("{}", "✗ broken".red()),
                LinkStatus::Wrong(t) => println!("{} → {}", "✗ wrong".red(), t.dimmed()),
                LinkStatus::Blocked => println!(
                    "{} → {}",
                    "✗ not linked".yellow(),
                    contract_tilde(&commands_link).dimmed()
                ),
            }
        }
```

4. Update `scan_all_sources` call (line 135-140) to pass `&central_commands`:
```rust
    let groups = skills::scan_all_sources(
        &expand_tilde(&config.central.source_dir),
        &central_skills,
        &central_agents,
        &central_commands,
        &config.central.source_repos,
    );
```

5. After `installed_agents` counting (line 146-150), add:
```rust
    let installed_commands: usize = groups
        .iter()
        .flat_map(|g| &g.commands)
        .filter(|c| c.install_status == skills::SkillInstallStatus::Installed)
        .count();
```

6. After the agents central display (line 159-163), add:
```rust
    println!(
        "Central commands: {} ({} installed)",
        contract_tilde(&central_commands),
        installed_commands,
    );
```

- [ ] **Step 2: Build to verify**

Run: `cargo build 2>&1 | head -40`

- [ ] **Step 3: Commit**

```bash
git add src/status.rs
git commit -m "feat: display commands link status in agm status"
```

---

### Task 7: TUI Tool View — Add Commands to CentralField and LinkField

**Files:**
- Modify: `src/tui/tool.rs`

- [ ] **Step 1: Add `Commands` variant to enums**

In `src/tui/tool.rs`:

1. Add `Commands` to `CentralField` (after `Agents`, line 37):
```rust
pub enum CentralField {
    Config,
    Prompt,
    Skills,
    Agents,
    Commands,
    Source,
}
```

2. Add `Commands` to `LinkField` (after `Agents`, line 46):
```rust
pub enum LinkField {
    Prompt,
    Skills,
    Agents,
    Commands,
}
```

- [ ] **Step 2: Add Commands row to `build_rows`**

In `build_rows()` (lines 107-112), add `Commands` central item after Agents:
```rust
        rows.push(ToolRow::CentralItem(CentralField::Agents));
        rows.push(ToolRow::CentralItem(CentralField::Commands));
        rows.push(ToolRow::CentralItem(CentralField::Source));
```

For per-tool link items (lines 138-142), add Commands after Agents:
```rust
                rows.push(ToolRow::LinkItem {
                    tool_key: key.clone(),
                    field: LinkField::Agents,
                });
                rows.push(ToolRow::LinkItem {
                    tool_key: key.clone(),
                    field: LinkField::Commands,
                });
```

- [ ] **Step 3: Add Commands to `get_link_paths`**

After the `LinkField::Agents` arm (line 641-645), add:
```rust
            LinkField::Commands => {
                let link = config_dir.join(&tool.commands_dir);
                let target = expand_tilde(&self.config.central.commands_source);
                (link, target, true, "commands")
            }
```

- [ ] **Step 4: Add Commands to `handle_blocked_link`**

In `handle_blocked_link` (around line 752-762), update the match to include Commands:
```rust
            let result = match field {
                LinkField::Agents => {
                    let agents_target = tool_target.join("agents");
                    skills::migrate_agents_dir_quiet(
                        link_path,
                        &agents_target,
                        central_dir,
                        tool_key,
                    )
                }
                LinkField::Commands => {
                    let commands_target = tool_target.join("commands");
                    skills::migrate_commands_dir_quiet(
                        link_path,
                        &commands_target,
                        central_dir,
                        tool_key,
                    )
                }
                _ => skills::migrate_tool_dir_quiet(link_path, &tool_target, central_dir, tool_key),
            };
```

- [ ] **Step 5: Add Commands to `recover_after_unlink`**

After the `LinkField::Agents` arm (line 916-952), add a `LinkField::Commands` arm:
```rust
            LinkField::Commands => {
                let commands_store = tool_store.join("commands");
                if commands_store.exists() {
                    match std::fs::rename(&commands_store, link_path) {
                        Ok(()) => {
                            self.log.push(
                                LogLevel::Info,
                                format!("[{}] Restored commands directory", tool_key),
                            );
                        }
                        Err(e) => {
                            self.log.push(
                                LogLevel::Warning,
                                format!("[{}] Failed to restore commands: {}", tool_key, e),
                            );
                        }
                    }
                    self.cleanup_empty_tool_store(&tool_store);
                } else {
                    match std::fs::create_dir_all(link_path) {
                        Ok(()) => {
                            self.log.push(
                                LogLevel::Info,
                                format!("[{}] Created empty commands directory", tool_key),
                            );
                        }
                        Err(e) => {
                            self.log.push(
                                LogLevel::Warning,
                                format!("[{}] Failed to create commands directory: {}", tool_key, e),
                            );
                        }
                    }
                }
            }
```

- [ ] **Step 6: Add Commands to `toggle_all_links`**

Update the `fields` array (line 1012):
```rust
        let fields = [LinkField::Prompt, LinkField::Skills, LinkField::Agents, LinkField::Commands];
```

- [ ] **Step 7: Add Commands to central rendering and path editing**

1. In the central field rendering (`CentralField::Agents =>` around line 1785), add after it:
```rust
                CentralField::Commands => (
                    "commands".to_string(),
                    contract_tilde(&expand_tilde(&config.central.commands_source)),
                ),
```

2. In the path editor field match (`CentralField::Agents =>` around line 412), add Commands:
```rust
                                CentralField::Commands => self.config.central.commands_source.clone(),
```

3. In the path save match (`CentralField::Agents =>` around line 1496), add:
```rust
            CentralField::Commands => self.config.central.commands_source = contracted.clone(),
```

4. In the popup label match (`CentralField::Agents =>` around line 2014), add:
```rust
            CentralField::Commands => "commands",
```

- [ ] **Step 8: Build to verify**

Run: `cargo build 2>&1 | head -40`

- [ ] **Step 9: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: add commands support to TUI tool view"
```

---

### Task 8: TUI Source View — Add Commands category

**Files:**
- Modify: `src/tui/source.rs`

- [ ] **Step 1: Add `Commands` to `Category` and `ListRow` enums**

In `src/tui/source.rs`:

1. Add `Commands` to `Category` (line 33-36):
```rust
enum Category {
    Skills,
    Agents,
    Commands,
}
```

2. Add `CommandItem` to `ListRow` (line 39-55):
```rust
    CommandItem {
        group_index: usize,
        command_index: usize,
    },
```

- [ ] **Step 2: Add `commands_dir` and `expanded_commands_sources` to App state**

In the `App` struct (around lines 77-100), add:
```rust
    commands_dir: PathBuf,
```
(after `agents_dir`)

```rust
    expanded_commands_sources: HashSet<usize>,
```
(after `expanded_agents_sources`)

In `App::new` (line 104-130):
1. Add `commands_dir: PathBuf` parameter.
2. Add `commands_dir,` to the struct initializer.
3. Add `expanded_commands_sources: HashSet::new(),` to the struct initializer.

- [ ] **Step 3: Update `rebuild_rows` to pass `expanded_commands_sources`**

Update `rebuild_rows` (line 141-148):
```rust
    fn rebuild_rows(&mut self) {
        self.rows = build_rows(
            &self.groups,
            &self.expanded_categories,
            &self.expanded_skills_sources,
            &self.expanded_agents_sources,
            &self.expanded_commands_sources,
        );
        self.apply_search_filter();
    }
```

- [ ] **Step 4: Update `refresh` to prune and scan commands**

Update `refresh` (line 158-169):
```rust
    fn refresh(&mut self) {
        let _ = skills::prune_broken_skills(&self.skills_dir);
        let _ = skills::prune_broken_agents(&self.agents_dir);
        let _ = skills::prune_broken_commands(&self.commands_dir);
        self.groups = skills::scan_all_sources(
            &self.source_dir,
            &self.skills_dir,
            &self.agents_dir,
            &self.commands_dir,
            &self.config.central.source_repos,
        );
        self.rebuild_rows();
        self.clamp_cursor();
    }
```

- [ ] **Step 5: Update search filter to include commands**

In `apply_search_filter` (around lines 228-291):

1. Add `matching_groups_commands` and `matching_commands` HashSets (after line 231):
```rust
        let mut matching_groups_commands: HashSet<usize> = HashSet::new();
        let mut matching_commands: HashSet<(usize, usize)> = HashSet::new();
```

2. Add commands matching loop (after agents loop, around line 245):
```rust
            for (ci, command) in group.commands.iter().enumerate() {
                if self.matcher.fuzzy_match(&command.name, query).is_some() {
                    matching_groups_commands.insert(gi);
                    matching_commands.insert((gi, ci));
                }
            }
```

3. Add `Category::Commands` arm in CategoryHeader match (around line 253-255):
```rust
                        Category::Commands => !matching_groups_commands.is_empty(),
```

4. Add `Category::Commands` arm in SourceHeader match (around line 265-267):
```rust
                        Category::Commands => matching_groups_commands.contains(group_index),
```

5. Add `CommandItem` match (after AgentItem, around line 281-288):
```rust
                ListRow::CommandItem {
                    group_index,
                    command_index,
                } => {
                    if matching_commands.contains(&(*group_index, *command_index)) {
                        result.push(i);
                    }
                }
```

- [ ] **Step 6: Update `toggle_item` to handle Commands**

In `toggle_item` (around lines 294-337):

1. Add `Category::Commands` to SourceHeader toggle (around line 313-315):
```rust
                    Category::Commands => &mut self.expanded_commands_sources,
```

2. Add `CommandItem` match arm (after AgentItem, around line 331-336):
```rust
            ListRow::CommandItem {
                group_index,
                command_index,
            } => {
                self.toggle_command(group_index, command_index);
            }
```

- [ ] **Step 7: Add `toggle_command` method**

After `toggle_agent` (line 432), add:

```rust
    fn toggle_command(&mut self, group_index: usize, command_index: usize) {
        let command = &self.groups[group_index].commands[command_index];
        let name = command.name.clone();
        let source_path = command.source_path.clone();
        match command.install_status {
            SkillInstallStatus::Installed => {
                match skills::uninstall_command(&name, &self.commands_dir) {
                    Ok(()) => {
                        self.groups[group_index].commands[command_index].install_status =
                            SkillInstallStatus::NotInstalled;
                        self.log
                            .push(super::log::LogLevel::Success, format!("Uninstalled {name}"));
                        self.set_status(format!("Uninstalled command {name}"));
                    }
                    Err(e) => {
                        self.log
                            .push(super::log::LogLevel::Error, format!("Uninstall error: {e}"));
                        self.set_status(format!("Error: {e}"));
                    }
                }
            }
            SkillInstallStatus::NotInstalled => {
                match skills::install_command(&name, &source_path, &self.commands_dir) {
                    Ok(()) => {
                        self.groups[group_index].commands[command_index].install_status =
                            SkillInstallStatus::Installed;
                        self.log
                            .push(super::log::LogLevel::Success, format!("Installed {name}"));
                        self.set_status(format!("Installed command {name}"));
                    }
                    Err(e) => {
                        self.log
                            .push(super::log::LogLevel::Error, format!("Install error: {e}"));
                        self.set_status(format!("Error: {e}"));
                    }
                }
            }
            SkillInstallStatus::Conflict => {
                self.log.push(
                    super::log::LogLevel::Warning,
                    format!("Conflict: command {name} from another source"),
                );
                self.set_status(format!("Conflict: command {name} from another source"));
            }
        }
    }
```

- [ ] **Step 8: Update `execute_delete` to pass `commands_dir`**

In `execute_delete` (line 455-477), update the `delete_source` call:
```rust
        match skills::delete_source(&group, &self.skills_dir, &self.agents_dir, &self.commands_dir) {
```

- [ ] **Step 9: Update `start_bulk_toggle` and `execute_bulk_toggle`**

In `start_bulk_toggle` (line 479-498), add `Category::Commands` arm:
```rust
            Category::Commands => group
                .commands
                .iter()
                .filter(|c| c.install_status != SkillInstallStatus::Conflict)
                .all(|c| c.install_status == SkillInstallStatus::Installed),
```

In `execute_bulk_toggle` (line 500-545), add `Category::Commands` arm:
```rust
            Category::Commands => {
                let len = self.groups[group_index].commands.len();
                for ci in 0..len {
                    let status = &self.groups[group_index].commands[ci].install_status;
                    if *status == SkillInstallStatus::Conflict {
                        continue;
                    }
                    let should_act = if install {
                        *status == SkillInstallStatus::NotInstalled
                    } else {
                        *status == SkillInstallStatus::Installed
                    };
                    if should_act {
                        self.toggle_command(group_index, ci);
                        count += 1;
                    }
                }
            }
```

Also update the kind label (around line 541-543):
```rust
        let kind = match category {
            Category::Skills => "skill(s)",
            Category::Agents => "agent(s)",
            Category::Commands => "command(s)",
        };
```

- [ ] **Step 10: Update `open_editor` to handle CommandItem**

In `open_editor` (around lines 548-589), add `CommandItem` match after `AgentItem`:
```rust
            ListRow::CommandItem {
                group_index,
                command_index,
            } => {
                let command = &self.groups[group_index].commands[command_index];
                if !command.source_path.exists() {
                    self.set_status("Command file not found");
                    return;
                }
                command.source_path.clone()
            }
```

- [ ] **Step 11: Update `show_info` to handle CommandItem**

In `show_info` (around lines 591-613), add `CommandItem` match:
```rust
            ListRow::CommandItem {
                group_index,
                command_index,
            } => self.build_command_info_lines(group_index, command_index),
```

- [ ] **Step 12: Add `build_command_info_lines` method**

After `build_agent_info_lines` (line 739), add:

```rust
    fn build_command_info_lines(&self, group_index: usize, command_index: usize) -> Vec<Line<'static>> {
        let group = &self.groups[group_index];
        let command = &group.commands[command_index];
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("Name: ", Style::default().fg(Color::Yellow)),
            Span::raw(command.name.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::Yellow)),
            Span::raw(group.name.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Path: ", Style::default().fg(Color::Yellow)),
            Span::raw(contract_tilde(&command.source_path).to_string()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{:?}", command.install_status)),
        ]));
        lines.push(Line::default());

        if command.source_path.exists() {
            lines.push(Line::from(Span::styled(
                format!(
                    "─── {} ───",
                    command
                        .source_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("command.md")
                ),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            match std::fs::read_to_string(&command.source_path) {
                Ok(content) => {
                    for line in content.lines().take(5000) {
                        lines.push(Line::from(line.to_string()));
                    }
                }
                Err(e) => {
                    lines.push(Line::from(format!("(error reading command file: {})", e)));
                }
            }
        }

        lines
    }
```

- [ ] **Step 13: Update `build_source_info_lines`**

In `build_source_info_lines` (around lines 741-816), after the agents count (line 771-774):
```rust
        lines.push(Line::from(vec![
            Span::styled("Commands: ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{}", group.commands.len())),
        ]));
```

After the agents list section (after line 813), add:
```rust
        // List commands
        if !group.commands.is_empty() {
            lines.push(Line::from(Span::styled(
                "Commands:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            for command in &group.commands {
                let status_icon = match command.install_status {
                    SkillInstallStatus::Installed => "✓",
                    SkillInstallStatus::NotInstalled => "○",
                    SkillInstallStatus::Conflict => "⚠",
                };
                lines.push(Line::from(format!("  {} {}", status_icon, command.name)));
            }
        }
```

- [ ] **Step 14: Update `expand_all`**

In `expand_all` (lines 818-830):
```rust
    fn expand_all(&mut self) {
        self.expanded_categories.insert(Category::Skills);
        self.expanded_categories.insert(Category::Agents);
        self.expanded_categories.insert(Category::Commands);
        for i in 0..self.groups.len() {
            if self.groups[i].skills.iter().any(|_| true) {
                self.expanded_skills_sources.insert(i);
            }
            if self.groups[i].agents.iter().any(|_| true) {
                self.expanded_agents_sources.insert(i);
            }
            if self.groups[i].commands.iter().any(|_| true) {
                self.expanded_commands_sources.insert(i);
            }
        }
        self.rebuild_rows();
        self.clamp_cursor();
    }
```

- [ ] **Step 15: Update `build_rows` function**

Update signature (lines 1165-1170):
```rust
fn build_rows(
    groups: &[SourceGroup],
    expanded_categories: &HashSet<Category>,
    expanded_skills_sources: &HashSet<usize>,
    expanded_agents_sources: &HashSet<usize>,
    expanded_commands_sources: &HashSet<usize>,
) -> Vec<ListRow> {
```

After the agents section (line 1225), add:
```rust
    // Commands section
    let has_commands = groups.iter().any(|g| !g.commands.is_empty());
    if has_commands {
        rows.push(ListRow::CategoryHeader {
            category: Category::Commands,
        });
        if expanded_categories.contains(&Category::Commands) {
            for (gi, group) in groups.iter().enumerate() {
                if group.commands.is_empty() {
                    continue;
                }
                rows.push(ListRow::SourceHeader {
                    category: Category::Commands,
                    group_index: gi,
                });
                if expanded_commands_sources.contains(&gi) {
                    for ci in 0..group.commands.len() {
                        rows.push(ListRow::CommandItem {
                            group_index: gi,
                            command_index: ci,
                        });
                    }
                }
            }
        }
    }
```

- [ ] **Step 16: Update rendering — CategoryHeader for Commands**

In the rendering function (around lines 1438-1467), add `Category::Commands` arm after `Category::Agents`:
```rust
                    Category::Commands => {
                        let total: usize = app.groups.iter().map(|g| g.commands.len()).sum();
                        let installed: usize = app
                            .groups
                            .iter()
                            .flat_map(|g| &g.commands)
                            .filter(|c| c.install_status == SkillInstallStatus::Installed)
                            .count();
                        (
                            format!("💬 Commands [{installed}/{total}]"),
                            app.expanded_categories.contains(&Category::Commands),
                        )
                    }
```

- [ ] **Step 17: Update rendering — SourceHeader for Commands**

In the SourceHeader rendering (around lines 1490-1504), add `Category::Commands` arm:
```rust
                    Category::Commands => {
                        let c = count_label(
                            &group.commands.iter().map(|_| 0u8).collect::<Vec<_>>(),
                            "command",
                        );
                        (c, app.expanded_commands_sources.contains(group_index))
                    }
```

- [ ] **Step 18: Update rendering — CommandItem**

After `AgentItem` rendering (around lines 1541-1560), add:
```rust
            ListRow::CommandItem {
                group_index,
                command_index,
            } => {
                let command = &app.groups[*group_index].commands[*command_index];
                let indices = if app.filtered_rows.is_some() && !app.search_query.is_empty() {
                    app.matcher
                        .fuzzy_indices(&command.name, &app.search_query)
                        .map(|(_, idx)| idx)
                } else {
                    None
                };
                render_item_line(
                    &command.name,
                    &command.install_status,
                    is_cursor,
                    ">",
                    indices.as_deref(),
                )
            }
```

- [ ] **Step 19: Update hints — add CommandItem**

In `build_source_hints` (around lines 1577-1620), add after AgentItem:
```rust
        Some(ListRow::CommandItem { .. }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" info  ")]);
            spans.extend([hint_key("i"), hint_text(" install  ")]);
            spans.extend([hint_key("e"), hint_text(" edit  ")]);
            spans.extend([hint_key("/"), hint_text(" search  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
```

- [ ] **Step 20: Update delete confirmation and bulk toggle prompts**

In delete confirmation (around line 1636), update count to include commands:
```rust
                    "Delete \"{}\" ({} skill(s), {} agent(s), {} command(s))? [y/N]",
                    g.name,
                    g.skills.len(),
                    g.agents.len(),
                    g.commands.len()
```

In bulk toggle prompt (around line 1653-1684), add `Category::Commands` arm:
```rust
                    Category::Commands => {
                        let c = g
                            .commands
                            .iter()
                            .filter(|c| {
                                c.install_status != SkillInstallStatus::Conflict
                                    && if *install {
                                        c.install_status == SkillInstallStatus::NotInstalled
                                    } else {
                                        c.install_status == SkillInstallStatus::Installed
                                    }
                            })
                            .count();
                        ("command(s)", c)
                    }
```

- [ ] **Step 21: Update `pub fn run` and UpdateAllDone handler**

In `pub fn run` (line 1740-1755):
1. Add `commands_dir` resolution:
```rust
    let commands_dir = expand_tilde(&config.central.commands_source);
```
2. Add prune:
```rust
    let _ = skills::prune_broken_commands(&commands_dir);
```
3. Update `scan_all_sources` call to include `&commands_dir`.
4. Update `App::new` call to include `commands_dir`.

In UpdateAllDone handler (lines 1812-1824), add `new_commands`:
```rust
                    TaskEvent::UpdateAllDone {
                        total,
                        updated,
                        new_skills,
                        new_agents,
                        new_commands,
                    } => {
                        app.log.push(
                            super::log::LogLevel::Success,
                            format!(
                            "Update complete: {} repos, {} updated, {} new skills, {} new agents, {} new commands",
                            total, updated, new_skills, new_agents, new_commands
                        ),
                        );
```

- [ ] **Step 22: Update spawn_update call**

Find where `background::spawn_update` is called in source.rs and add `commands_dir`:
```rust
background::spawn_update(skills_dir, agents_dir, commands_dir, source_dir)
```
(Update the clone/pass to include `self.commands_dir.clone()`.)

- [ ] **Step 23: Build to verify**

Run: `cargo build 2>&1 | head -60`

- [ ] **Step 24: Commit**

```bash
git add src/tui/source.rs
git commit -m "feat: add commands support to TUI source view"
```

---

### Task 9: Fix Remaining Compilation Errors

**Files:**
- Any files with remaining compilation errors

- [ ] **Step 1: Full build and fix**

Run: `cargo build 2>&1`

Fix any remaining compilation errors. Common issues:
- Missing `commands` field in `SourceGroup` initializers (tests)
- Changed function signatures not updated in callers
- Missing match arms in exhaustive matches

- [ ] **Step 2: Run all tests**

Run: `cargo test 2>&1`

Fix any test failures.

- [ ] **Step 3: Commit fixes**

```bash
git add -A
git commit -m "fix: resolve compilation errors for commands support"
```

---

### Task 10: Unit Tests for Commands

**Files:**
- Modify: `src/skills.rs` (add tests at bottom of file)

- [ ] **Step 1: Add `test_scan_commands`**

At the bottom of `src/skills.rs` (inside `#[cfg(test)] mod tests`), add:

```rust
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
```

- [ ] **Step 2: Add `test_install_uninstall_command`**

```rust
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
```

- [ ] **Step 3: Add `test_install_command_conflict`**

```rust
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
```

- [ ] **Step 4: Add `test_prune_broken_commands`**

```rust
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
```

- [ ] **Step 5: Add `test_migrate_commands_dir_quiet`**

```rust
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
```

- [ ] **Step 6: Run tests**

Run: `cargo test 2>&1`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/skills.rs
git commit -m "test: add unit tests for commands support"
```

---

### Task 11: Final Verification

- [ ] **Step 1: Full build**

Run: `cargo build 2>&1`
Expected: Clean build, no errors.

- [ ] **Step 2: Full test suite**

Run: `cargo test 2>&1`
Expected: All tests pass.

- [ ] **Step 3: Commit any remaining fixes**

```bash
git add -A
git commit -m "chore: final cleanup for commands support"
```
