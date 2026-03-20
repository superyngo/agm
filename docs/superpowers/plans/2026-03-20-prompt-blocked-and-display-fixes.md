# Prompt Blocked Handling & Display Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix three related issues — empty prompt file gap, blocked status path display, and platform-native path separators.

**Architecture:** Three independent, surgical changes to existing files. No new modules or public API changes. Each fix is self-contained and testable independently.

**Tech Stack:** Rust, std::path, std::fs, colored crate

**Spec:** `docs/superpowers/specs/2026-03-20-prompt-blocked-and-display-fixes-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/paths.rs:74-81` | Modify | Fix `contract_tilde()` to use platform-native separators |
| `src/paths.rs:131-135` | Modify | Update `test_contract_tilde` to be platform-aware |
| `src/status.rs:69` | Modify | Add path arrow to prompt `Blocked` display |
| `src/status.rs:90` | Modify | Add path arrow to skills `Blocked` display |
| `src/main.rs:550-573` | Modify | Add `else` branch to remove empty prompt files |

---

### Task 1: Platform-native path separators in `contract_tilde`

**Why first:** This is a leaf function with no dependencies on the other changes. Fixing it first means all subsequent path displays in Task 2 will already use correct separators.

**Files:**
- Modify: `src/paths.rs:74-81` — `contract_tilde()` function
- Modify: `src/paths.rs:131-135` — `test_contract_tilde` test

- [ ] **Step 1: Update the `test_contract_tilde` test to expect platform-native separators**

Open `src/paths.rs` and replace the test at lines 131-135:

```rust
// Before
#[test]
fn test_contract_tilde() {
    let home = dirs::home_dir().unwrap();
    let path = home.join(".config/agm");
    assert_eq!(contract_tilde(&path), "~/.config/agm");
}

// After
#[test]
fn test_contract_tilde() {
    let home = dirs::home_dir().unwrap();
    let path = home.join(".config").join("agm");
    let sep = std::path::MAIN_SEPARATOR;
    assert_eq!(
        contract_tilde(&path),
        format!("~{}.config{}agm", sep, sep)
    );
}
```

Note: Changed `home.join(".config/agm")` to `home.join(".config").join("agm")` so path components use native separators on all platforms.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test test_contract_tilde -- --nocapture`

Expected: FAIL — the function still produces `~/` prefix while the test expects platform-native separator.

- [ ] **Step 3: Implement the fix in `contract_tilde`**

Open `src/paths.rs` and replace the `contract_tilde` function (lines 74-81):

```rust
// Before
pub fn contract_tilde(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

// After
pub fn contract_tilde(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            let display = format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display());
            #[cfg(windows)]
            let display = display.replace('/', "\\");
            return display;
        }
    }
    path.display().to_string()
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test test_contract_tilde -- --nocapture`

Expected: PASS

- [ ] **Step 5: Run all paths tests to check for regressions**

Run: `cargo test paths -- --nocapture`

Expected: All paths tests pass. The `test_contract_no_home` test uses `/tmp/foo` which doesn't start with home, so it's unaffected.

Note: On Windows, the existing `test_contract_no_home` test uses `/tmp/foo` which is a Unix path. This test should still pass because `strip_prefix` will fail (no home match) and it falls through to `path.display().to_string()`.

- [ ] **Step 6: Commit**

```bash
git add src/paths.rs
git commit -m "fix: use platform-native path separators in contract_tilde

On Windows, contract_tilde now produces ~\.claude\CLAUDE.md instead of
~/.claude\CLAUDE.md (mixed separators). Linux output unchanged.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 2: Show path in Blocked status display

**Files:**
- Modify: `src/status.rs:69` — prompt Blocked display
- Modify: `src/status.rs:90` — skills Blocked display

- [ ] **Step 1: Update prompt Blocked display**

Open `src/status.rs` and replace line 69:

```rust
// Before
LinkStatus::Blocked => println!("{}", "✗ blocked".red()),

// After
LinkStatus::Blocked => println!(
    "{} → {}",
    "✗ blocked".red(),
    contract_tilde(&prompt_link).dimmed()
),
```

- [ ] **Step 2: Update skills Blocked display**

In the same file, replace line 90:

```rust
// Before
LinkStatus::Blocked => println!("{}", "✗ blocked".red()),

// After
LinkStatus::Blocked => println!(
    "{} → {}",
    "✗ blocked".red(),
    contract_tilde(&skills_link).dimmed()
),
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`

Expected: Build succeeds with no errors or warnings in status.rs.

- [ ] **Step 4: Manual verification**

Run: `cargo run -- status`

Verify that if any tool has a blocked prompt or skills, the output now shows the path:
```
prompt   ✗ blocked → ~\.claude\CLAUDE.md
```
(If no tool is currently blocked, that's fine — the compile check is sufficient.)

- [ ] **Step 5: Commit**

```bash
git add src/status.rs
git commit -m "fix: show file path in blocked status display

Previously 'blocked' status showed no path, making it unclear which file
was blocking. Now shows the arrow and path like other statuses.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 3: Fix empty prompt file gap in link command

**Files:**
- Modify: `src/main.rs:550-573` — prompt linking pre-processing

- [ ] **Step 1: Add `else` branch for empty files**

Open `src/main.rs` and find the prompt linking block (around line 550-572). The current code:

```rust
} else {
    // Regular file (not a link)
    let content = fs::read_to_string(&prompt_link)?;
    if !content.trim().is_empty() {
        if yes
            || prompt_yes_no(&format!(
                "Existing prompt file found at {}. Backup and create link?",
                paths::contract_tilde(&prompt_link)
            ))
        {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let backup_path =
                prompt_link.with_extension(format!("{}.bak", timestamp));
            fs::rename(&prompt_link, &backup_path)?;
            println!(
                "  {} Backed up prompt to {}",
                " ok ".green(),
                paths::contract_tilde(&backup_path)
            );
        } else {
            println!("  {} Skipping prompt link", "skip".yellow());
            continue;
        }
    }
}
```

Add an `else` branch after the `if !content.trim().is_empty()` block:

```rust
} else {
    // Regular file (not a link)
    let content = fs::read_to_string(&prompt_link)?;
    if !content.trim().is_empty() {
        if yes
            || prompt_yes_no(&format!(
                "Existing prompt file found at {}. Backup and create link?",
                paths::contract_tilde(&prompt_link)
            ))
        {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let backup_path =
                prompt_link.with_extension(format!("{}.bak", timestamp));
            fs::rename(&prompt_link, &backup_path)?;
            println!(
                "  {} Backed up prompt to {}",
                " ok ".green(),
                paths::contract_tilde(&backup_path)
            );
        } else {
            println!("  {} Skipping prompt link", "skip".yellow());
            continue;
        }
    } else {
        // Empty file — safe to remove without backup
        fs::remove_file(&prompt_link)?;
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`

Expected: Build succeeds.

- [ ] **Step 3: Run all tests**

Run: `cargo test`

Expected: All tests pass (this code path has no unit tests — it's integration-level logic).

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "fix: remove empty prompt files before linking

Previously, empty prompt files fell through without being removed,
causing create_link to hit the Blocked path and skip with a warning.
Now empty files are removed (no backup needed) so the link can be created.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 4: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`

Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy`

Expected: No new warnings.

- [ ] **Step 3: Verify status display**

Run: `cargo run -- status`

Verify paths use native separators and blocked items show file paths.
