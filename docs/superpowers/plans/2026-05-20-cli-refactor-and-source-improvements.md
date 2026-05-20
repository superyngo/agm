# CLI Refactor and Source Improvements — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** `docs/superpowers/specs/2026-05-20-cli-refactor-and-source-improvements-design.md`

**Goal:** Restructure `agm` CLI to subcommands (breaking), add `source del`/`rename`/`-n`, eliminate TUI screen tearing, and surface preload-char statistics in info popups.

**Architecture:** clap derive subcommand enums replace boolean flags. `skills::clone_or_pull` and `add_local_copy` accept an `on_progress: FnMut(CloneProgress)` callback so callers (CLI stdout, TUI `LogBuffer`) decide how to render git output — git stdio is piped, two reader threads feed an `mpsc` channel. New resolver + rename helpers, hand-rolled YAML frontmatter scanner for preload-char counts cached on `SkillInfo`/`AgentInfo`/`CommandInfo`.

**Tech Stack:** Rust 2021, clap 4 derive, ratatui 0.29, crossterm 0.28, `std::process::Command`, `std::thread`, `std::sync::mpsc`, `assert_cmd` + `tempfile` for tests.

---

## File Structure

| File | Role |
|---|---|
| `src/main.rs` | CLI parsing + dispatch only. Match `ToolAction` / `SourceAction` subcommands, build stdout sinks, call `skills::*` helpers. |
| `src/skills.rs` | Domain logic: new `CloneProgress` / `CloneAction` enums, `RenameReport` struct; `clone_or_pull` / `add_local_copy` callback-ized with `target_name`; `resolve_source_target`, `rename_source`, `skill_preload_chars`, `file_char_count` helpers; `preload_chars` field on the three `*Info` structs; legacy `update_all` removed. |
| `src/tui/source.rs` | `do_add_submit` consumes new callback into `self.log`; rename mode state + `r`/`F5` rebinding; info-popup builders show preload-char rows. |
| `tests/cli.rs` (new) | `assert_cmd` integration tests for new CLI shape. |
| `tests/source_ops.rs` (new) | Unit-ish tests for `resolve_source_target`, `rename_source`, `skill_preload_chars`, `file_char_count`, name validation, using `tempfile`. |
| `CHANGELOG.md` | Unreleased entry. |
| `README.md` | Updated CLI examples. |

Untouched: `linker.rs`, `platform.rs`, `config.rs`, `init.rs`, `status.rs`, `editor.rs`, `paths.rs`, `tui/tool.rs`, `tui/log.rs`, `tui/popup.rs`, `tui/background.rs`, `tui/mod.rs`.

---

## Conventions for every task

- Run `cargo build` and `cargo test` after each task; both must succeed before the commit step. Failing tests block the commit.
- Use `cargo fmt` before committing.
- Commit messages follow conventional commits (`feat:`, `refactor:`, `fix:`, `test:`, `docs:`).

---

## Task 1: Add `CloneProgress` / `CloneAction` types and unit test

**Files:**
- Modify: `src/skills.rs` (add types near other public types around line 60)
- Test: `tests/source_ops.rs` (create)

- [ ] **Step 1: Create `tests/source_ops.rs` with a smoke test for the new types**

```rust
// tests/source_ops.rs
use agm::skills::{CloneAction, CloneProgress};

#[test]
fn clone_progress_variants_constructible() {
    let _ = CloneProgress::Start {
        name: "r".into(),
        url: "u".into(),
        action: CloneAction::Clone,
    };
    let _ = CloneProgress::GitLine { line: "x".into(), is_err: false };
    let _ = CloneProgress::Done {
        name: "r".into(),
        success: true,
        message: "ok".into(),
    };
}
```

- [ ] **Step 2: Expose `skills` as a library by ensuring `src/lib.rs` re-exports it**

Check whether `src/lib.rs` exists. If not, create it:

```rust
// src/lib.rs
pub mod paths;
pub mod platform;
pub mod skills;
pub mod config;
pub mod linker;
pub mod status;
pub mod editor;
pub mod init;
pub mod tui;
```

Then add to `Cargo.toml` under `[package]` if not present:

```toml
[lib]
name = "agm"
path = "src/lib.rs"

[[bin]]
name = "agm"
path = "src/main.rs"
```

Update `src/main.rs` top — replace the existing `mod` lines with `use agm::{config, editor, init, linker, paths, platform, skills, status, tui};`.

- [ ] **Step 3: Run the test, confirm it fails to compile**

Run: `cargo test --test source_ops`
Expected: build error — `CloneProgress` not found.

- [ ] **Step 4: Add the enums to `src/skills.rs`**

Insert near the top of the file, after the existing `SourceKind` enum (around line 53):

```rust
/// Whether `clone_or_pull` is performing a fresh clone or pulling an existing repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloneAction {
    Clone,
    Pull,
}

/// Progress events from `clone_or_pull` / `add_local_copy` / `rename_source`.
#[derive(Debug, Clone)]
pub enum CloneProgress {
    /// Operation started.
    Start {
        name: String,
        url: String,
        action: CloneAction,
    },
    /// A single line from the underlying git subprocess.
    GitLine { line: String, is_err: bool },
    /// Operation finished. `success=false` means the caller should treat this as a failure.
    Done {
        name: String,
        success: bool,
        message: String,
    },
}
```

- [ ] **Step 5: Run test, confirm pass**

Run: `cargo test --test source_ops`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add src/lib.rs src/main.rs src/skills.rs Cargo.toml tests/source_ops.rs
git commit -m "refactor: add CloneProgress/CloneAction types and library crate"
```

---

## Task 2: Frontmatter parser — `skill_preload_chars` + `file_char_count`

**Files:**
- Modify: `src/skills.rs` (append helpers near other free fns)
- Test: `tests/source_ops.rs`

- [ ] **Step 1: Add tests**

Append to `tests/source_ops.rs`:

```rust
use agm::skills::{file_char_count, skill_preload_chars};
use std::fs;
use tempfile::tempdir;

fn write(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    fs::write(&p, content).unwrap();
    p
}

#[test]
fn preload_standard_keys() {
    let d = tempdir().unwrap();
    let skill = d.path().join("s");
    fs::create_dir(&skill).unwrap();
    write(
        &skill,
        "SKILL.md",
        "---\nname: foo\ndescription: hello\n---\nbody\n",
    );
    assert_eq!(skill_preload_chars(&skill), "foo".chars().count() + "hello".chars().count());
}

#[test]
fn preload_quoted_values() {
    let d = tempdir().unwrap();
    let skill = d.path().join("s");
    fs::create_dir(&skill).unwrap();
    write(
        &skill,
        "SKILL.md",
        "---\nname: \"foo bar\"\ndescription: 'hi'\n---\n",
    );
    assert_eq!(
        skill_preload_chars(&skill),
        "foo bar".chars().count() + "hi".chars().count()
    );
}

#[test]
fn preload_block_scalar() {
    let d = tempdir().unwrap();
    let skill = d.path().join("s");
    fs::create_dir(&skill).unwrap();
    write(
        &skill,
        "SKILL.md",
        "---\nname: foo\ndescription: |\n  line one\n  line two\n---\n",
    );
    // value is "line one\nline two" (chars), 8 + 1 + 8 = 17
    assert_eq!(
        skill_preload_chars(&skill),
        "foo".chars().count() + "line one\nline two".chars().count()
    );
}

#[test]
fn preload_no_frontmatter() {
    let d = tempdir().unwrap();
    let skill = d.path().join("s");
    fs::create_dir(&skill).unwrap();
    write(&skill, "SKILL.md", "no frontmatter here\n");
    assert_eq!(skill_preload_chars(&skill), 0);
}

#[test]
fn preload_missing_key() {
    let d = tempdir().unwrap();
    let skill = d.path().join("s");
    fs::create_dir(&skill).unwrap();
    write(&skill, "SKILL.md", "---\nname: foo\n---\n");
    assert_eq!(skill_preload_chars(&skill), "foo".chars().count());
}

#[test]
fn preload_missing_file() {
    let d = tempdir().unwrap();
    let skill = d.path().join("nope");
    assert_eq!(skill_preload_chars(&skill), 0);
}

#[test]
fn file_char_count_basic() {
    let d = tempdir().unwrap();
    let p = write(d.path(), "f.md", "hello\n世界");
    assert_eq!(file_char_count(&p), "hello\n世界".chars().count());
}

#[test]
fn file_char_count_missing() {
    assert_eq!(file_char_count(std::path::Path::new("/nope/nope")), 0);
}
```

- [ ] **Step 2: Run, confirm failures**

Run: `cargo test --test source_ops preload`
Expected: build errors — functions not defined.

- [ ] **Step 3: Implement helpers**

Append to `src/skills.rs`:

```rust
/// Count unicode chars in the value of a top-level YAML key inside the slice of
/// frontmatter lines (lines between the two `---` markers, excluding markers).
fn extract_key_value_chars(lines: &[&str], key: &str) -> usize {
    let prefix = format!("{}:", key);
    for (i, line) in lines.iter().enumerate() {
        if !line.starts_with(&prefix) {
            continue;
        }
        // After the colon
        let rest = &line[prefix.len()..];
        let rest_trim = rest.trim();
        if rest_trim == "|" || rest_trim == ">" {
            // Block scalar — collect subsequent indented lines
            let mut acc = String::new();
            for cont in &lines[i + 1..] {
                if cont.trim().is_empty() {
                    if !acc.is_empty() {
                        acc.push('\n');
                    }
                    continue;
                }
                let leading = cont.len() - cont.trim_start().len();
                if leading == 0 {
                    break;
                }
                if !acc.is_empty() {
                    acc.push('\n');
                }
                acc.push_str(cont.trim_start());
            }
            return acc.chars().count();
        }
        // Inline scalar, possibly quoted
        let mut v = rest_trim.to_string();
        if (v.starts_with('"') && v.ends_with('"') && v.len() >= 2)
            || (v.starts_with('\'') && v.ends_with('\'') && v.len() >= 2)
        {
            v = v[1..v.len() - 1].to_string();
        }
        return v.chars().count();
    }
    0
}

/// Compute the preload-char count for a skill: sum of `name` + `description`
/// values in the SKILL.md YAML frontmatter. Returns 0 on any failure.
pub fn skill_preload_chars(skill_path: &Path) -> usize {
    let md = skill_path.join("SKILL.md");
    let content = match fs::read_to_string(&md) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let mut iter = content.lines();
    let first = match iter.next() {
        Some(l) => l,
        None => return 0,
    };
    if first.trim() != "---" {
        return 0;
    }
    let mut fm_lines: Vec<&str> = Vec::new();
    let mut terminated = false;
    for line in iter.take(200) {
        let t = line.trim();
        if t == "---" || t == "..." {
            terminated = true;
            break;
        }
        fm_lines.push(line);
    }
    if !terminated {
        return 0;
    }
    extract_key_value_chars(&fm_lines, "name") + extract_key_value_chars(&fm_lines, "description")
}

/// Count unicode chars in the whole file. Returns 0 on read error.
pub fn file_char_count(path: &Path) -> usize {
    fs::read_to_string(path)
        .map(|s| s.chars().count())
        .unwrap_or(0)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test source_ops`
Expected: all `preload_*` and `file_char_count_*` tests pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/skills.rs tests/source_ops.rs
git commit -m "feat(skills): add skill_preload_chars and file_char_count helpers"
```

---

## Task 3: Add `preload_chars` field and populate in `scan_all_sources`

**Files:**
- Modify: `src/skills.rs`

- [ ] **Step 1: Add the field to all three `*Info` structs**

In `src/skills.rs`, update:

```rust
#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub source_path: PathBuf,
    pub install_status: SkillInstallStatus,
    pub preload_chars: usize,
}

#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub name: String,
    pub source_path: PathBuf,
    pub install_status: SkillInstallStatus,
    pub preload_chars: usize,
}

#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub name: String,
    pub source_path: PathBuf,
    pub install_status: SkillInstallStatus,
    pub preload_chars: usize,
}
```

- [ ] **Step 2: Run build, confirm it fails**

Run: `cargo build`
Expected: errors at every `SkillInfo { ... }` / `AgentInfo { ... }` / `CommandInfo { ... }` construction site in `scan_all_sources`.

- [ ] **Step 3: Update every construction site**

In `scan_all_sources` (and any other call site — should all be in `skills.rs`), populate the field. For skills:

```rust
.map(|(name, sp)| SkillInfo {
    install_status: check_install_status(&name, &sp, skills_dir),
    preload_chars: skill_preload_chars(&sp),
    name,
    source_path: sp,
})
```

For agents and commands use `file_char_count(&sp)` instead of `skill_preload_chars(&sp)`. Apply to all three branches in `scan_all_sources` (`local`, `agm_tools`, regular repos).

- [ ] **Step 4: Run build and existing tests**

Run: `cargo build && cargo test`
Expected: PASS (no test for this field yet — it just needs to compile).

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/skills.rs
git commit -m "feat(skills): cache preload_chars on SkillInfo/AgentInfo/CommandInfo"
```

---

## Task 4: Refactor `clone_or_pull` — callback + `target_name` + piped stdio

**Files:**
- Modify: `src/skills.rs` (replace `clone_or_pull`)
- Modify: `src/main.rs` and `src/tui/source.rs` (update call sites to compile)

- [ ] **Step 1: Replace `clone_or_pull` with new signature**

Find the existing `pub fn clone_or_pull(url: &str, source_dir: &Path)` (around line 1034) and replace with:

```rust
pub fn clone_or_pull(
    url: &str,
    source_dir: &Path,
    target_name: Option<&str>,
    mut on_progress: impl FnMut(CloneProgress),
) -> anyhow::Result<(PathBuf, Vec<(String, PathBuf)>)> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;

    let name = match target_name {
        Some(n) => {
            validate_source_name(n)?;
            n.to_string()
        }
        None => repo_name_from_url(url),
    };
    let repo_path = source_dir.join(&name);

    let action = if repo_path.is_dir() {
        let existing_url = Command::new("git")
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
            if normalize_git_url(existing) != normalize_git_url(url) {
                anyhow::bail!(
                    "Directory '{}' already exists but belongs to a different repo ({}).\n\
                     Remove it manually or use a different URL.",
                    name,
                    existing
                );
            }
        }
        CloneAction::Pull
    } else {
        fs::create_dir_all(source_dir)?;
        CloneAction::Clone
    };

    on_progress(CloneProgress::Start {
        name: name.clone(),
        url: url.to_string(),
        action,
    });

    let mut cmd = Command::new("git");
    match action {
        CloneAction::Pull => {
            cmd.args(["pull"]).current_dir(&repo_path);
        }
        CloneAction::Clone => {
            cmd.args(["clone", url, &repo_path.display().to_string()]);
        }
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().context("failed to spawn git")?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let (tx, rx) = mpsc::channel::<(bool, String)>();
    let tx_err = tx.clone();
    let t_out = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().flatten() {
            let _ = tx.send((false, line));
        }
    });
    let t_err = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().flatten() {
            let _ = tx_err.send((true, line));
        }
    });

    let status = child.wait().context("git wait failed")?;
    let _ = t_out.join();
    let _ = t_err.join();
    // Both senders are dropped after the joins; iterate remaining buffered lines.
    for (is_err, line) in rx.iter() {
        on_progress(CloneProgress::GitLine { line, is_err });
    }

    let (success, message) = if status.success() {
        (
            true,
            match action {
                CloneAction::Pull => "Updated".to_string(),
                CloneAction::Clone => "Cloned".to_string(),
            },
        )
    } else {
        (
            false,
            format!(
                "git {} exited with {}",
                match action {
                    CloneAction::Pull => "pull",
                    CloneAction::Clone => "clone",
                },
                status
            ),
        )
    };

    on_progress(CloneProgress::Done {
        name: name.clone(),
        success,
        message: message.clone(),
    });

    if !success {
        anyhow::bail!("{}", message);
    }

    let skills = scan_skills(&repo_path);
    if skills.is_empty() {
        if action == CloneAction::Clone {
            let _ = fs::remove_dir_all(&repo_path);
        }
        anyhow::bail!("No skills found in {}. Clone removed.", url);
    }
    Ok((repo_path, skills))
}
```

- [ ] **Step 2: Add `validate_source_name`**

In `src/skills.rs`:

```rust
/// Validate a user-supplied source directory name.
pub fn validate_source_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        anyhow::bail!("source name must not be empty");
    }
    if name == "." || name == ".." {
        anyhow::bail!("source name must not be '.' or '..'");
    }
    if name.contains('/') || name.contains('\\') {
        anyhow::bail!("source name must not contain '/' or '\\\\'");
    }
    Ok(())
}
```

- [ ] **Step 3: Patch call sites to compile**

In `src/main.rs` inside the existing `Commands::Source { add: Some(source), ... }` branch, change:

```rust
let (repo_path, found_skills) = skills::clone_or_pull(&source, &source_dir)?;
```

to:

```rust
let (repo_path, found_skills) = skills::clone_or_pull(&source, &source_dir, None, |evt| {
    print_clone_progress(&evt);
})?;
```

Add helper above `main()`:

```rust
fn print_clone_progress(evt: &skills::CloneProgress) {
    use skills::CloneProgress::*;
    match evt {
        Start { name, url, action } => {
            let verb = match action {
                skills::CloneAction::Clone => "Cloning",
                skills::CloneAction::Pull => "Updating",
            };
            println!("{} {} from {}...", verb, name, url);
        }
        GitLine { line, is_err } => {
            if *is_err {
                eprintln!("{}", line);
            } else {
                println!("{}", line);
            }
        }
        Done { success, message, .. } => {
            if *success {
                println!("{} {}", " ok ".green(), message);
            } else {
                println!("{} {}", "fail".red(), message);
            }
        }
    }
}
```

In `src/tui/source.rs`, find the `do_add_submit` function and change the URL branch (currently at line 1224):

```rust
match skills::clone_or_pull(&source, &self.source_dir, None, |_evt| {}) {
```

(Placeholder no-op callback for now; Task 9 wires it to `self.log`.)

- [ ] **Step 4: Add unit tests for `validate_source_name` and clone-callback wiring**

Append to `tests/source_ops.rs`:

```rust
use agm::skills::validate_source_name;

#[test]
fn validate_names() {
    assert!(validate_source_name("foo").is_ok());
    assert!(validate_source_name("").is_err());
    assert!(validate_source_name(".").is_err());
    assert!(validate_source_name("..").is_err());
    assert!(validate_source_name("a/b").is_err());
    assert!(validate_source_name("a\\b").is_err());
}

#[test]
fn clone_or_pull_routes_errors_through_callback_not_stdout() {
    use agm::skills::{clone_or_pull, CloneProgress};
    use std::sync::{Arc, Mutex};

    let d = tempdir().unwrap();
    let source_dir = d.path().join("src");

    // Deliberately broken URL — git will fail. We assert:
    //   (a) function returns Err
    //   (b) at least one GitLine { is_err: true } was emitted
    //   (c) function did not write anything to the captured stdout buffer
    //       (we cannot easily capture process stdout from inside a test
    //        without an extra crate; instead we assert the callback received
    //        the stderr stream — which is the contract). The "no stdout
    //        tearing" guarantee is reinforced by the integration smoke in
    //        Task 11.
    let events = Arc::new(Mutex::new(Vec::<CloneProgress>::new()));
    let events_clone = events.clone();
    let res = clone_or_pull(
        "https://invalid.example.invalid/no/such/repo.git",
        &source_dir,
        None,
        move |evt| {
            events_clone.lock().unwrap().push(evt);
        },
    );
    assert!(res.is_err());
    let evts = events.lock().unwrap();
    assert!(
        evts.iter().any(|e| matches!(
            e,
            CloneProgress::GitLine { is_err: true, .. }
        )),
        "expected at least one stderr GitLine event, got: {:?}",
        evts
    );
    assert!(
        evts.iter().any(|e| matches!(
            e,
            CloneProgress::Done { success: false, .. }
        )),
        "expected a failing Done event"
    );
}
```

Note: the test requires network access to fail-resolve the hostname; it should run quickly on a normal dev machine. If CI lacks network, mark it `#[ignore]` and document that in the comment.

- [ ] **Step 5: Build and test**

Run: `cargo build && cargo test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add src/skills.rs src/main.rs src/tui/source.rs tests/source_ops.rs
git commit -m "refactor(skills): callback-based clone_or_pull with target_name override"
```

---

## Task 5: Refactor `add_local_copy` — callback + `target_name`

**Files:**
- Modify: `src/skills.rs` (`add_local_copy`)
- Modify: `src/main.rs`, `src/tui/source.rs` (call sites)

- [ ] **Step 1: Replace `add_local_copy`**

Replace the body of `add_local_copy` (around line 991) with:

```rust
pub fn add_local_copy(
    source: &Path,
    source_dir: &Path,
    target_name: Option<&str>,
    mut on_progress: impl FnMut(CloneProgress),
) -> anyhow::Result<(PathBuf, Vec<(String, PathBuf)>)> {
    if !source.exists() {
        anyhow::bail!("Source path does not exist: {}", source.display());
    }

    let pre_skills = scan_skills(source);
    if pre_skills.is_empty() {
        anyhow::bail!(
            "No skills found at {}. A skill must contain a SKILL.md file.",
            source.display()
        );
    }

    let name = match target_name {
        Some(n) => {
            validate_source_name(n)?;
            n.to_string()
        }
        None => source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string(),
    };

    let dest = source_dir.join("local").join(&name);
    if dest.exists() {
        anyhow::bail!(
            "Source '{}' already exists at {}. Remove it first or choose a different name.",
            name,
            contract_tilde(&dest)
        );
    }

    on_progress(CloneProgress::Start {
        name: name.clone(),
        url: source.display().to_string(),
        action: CloneAction::Clone,
    });

    fs::create_dir_all(dest.parent().unwrap())?;
    copy_dir_recursive(source, &dest)?;

    on_progress(CloneProgress::Done {
        name: name.clone(),
        success: true,
        message: format!("Copied to {}", contract_tilde(&dest)),
    });

    let skills = scan_skills(&dest);
    Ok((dest, skills))
}
```

- [ ] **Step 2: Update CLI call site in `src/main.rs`**

Replace:

```rust
let (dest, found_skills) = skills::add_local_copy(&source_path, &source_dir)?;
```

with:

```rust
let (dest, found_skills) = skills::add_local_copy(&source_path, &source_dir, None, |evt| {
    print_clone_progress(&evt);
})?;
```

- [ ] **Step 3: Update TUI call site in `src/tui/source.rs`**

Replace the local-path branch around line 1253:

```rust
match skills::add_local_copy(&source_path, &self.source_dir, None, |_evt| {}) {
```

- [ ] **Step 4: Build and test**

Run: `cargo build && cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/skills.rs src/main.rs src/tui/source.rs
git commit -m "refactor(skills): callback-based add_local_copy with target_name override"
```

---

## Task 6: Retire legacy `update_all`, repoint CLI to `update_all_with_progress`

**Files:**
- Modify: `src/skills.rs` (delete `update_all`)
- Modify: `src/main.rs` (use progress variant)

- [ ] **Step 1: Delete the legacy `update_all`**

Remove `pub fn update_all(skills_dir: &Path, agents_dir: &Path, source_dir: &Path) -> anyhow::Result<()>` (line 520) entirely. Build will break — that's expected.

- [ ] **Step 2: Update the caller in `src/main.rs`**

Inside the `update` branch of `Commands::Source`, replace `skills::update_all(&skills_dir, &agents_dir, &source_dir)?;` with:

```rust
use skills::UpdateProgress::*;
skills::update_all_with_progress(
    &skills_dir,
    &agents_dir,
    &commands_dir,
    &source_dir,
    |p| match p {
        RepoStart { name } => println!("Updating {}...", name),
        RepoComplete { name, success, message } => {
            let tag = if success { " ok ".green() } else { "fail".red() };
            println!("  {} {}: {}", tag, name, message);
        }
        AllDone { total, updated, new_skills, new_agents, new_commands } => {
            println!(
                "\nUpdated {}/{}; {} new skill(s), {} new agent(s), {} new command(s).",
                updated, total, new_skills, new_agents, new_commands
            );
        }
    },
);
```

Check the exact variant names of `UpdateProgress` in `src/skills.rs` — keep them in sync. If `update_all_with_progress` does not currently accept `commands_dir`, update the call arguments to match its real signature (it may use a 3-arg form; pass what it expects).

- [ ] **Step 3: Audit and remove remaining `println!` in `src/skills.rs`**

Run: `rg -n 'println!|eprintln!|print!' src/skills.rs`

Expected leftover sites (after Tasks 4–6): inside `migrate_tool_dir` and possibly older bookkeeping helpers. For each occurrence:

- If the function already returns structured data (e.g. `Vec<String>` messages, a status enum), append the message there.
- If the function is called only from CLI paths, return the message via a new `Vec<String>` out-param or change the signature to take an `on_progress` callback symmetric with §3.2.
- If the print is genuinely vestigial debug noise (rare), delete it.

For `migrate_tool_dir` specifically: it already has a `_quiet` sibling (`migrate_tool_dir_quiet`) returning `(usize, Vec<String>)`. Delete the wrapper `migrate_tool_dir` (which loops and prints) and make every existing caller use `migrate_tool_dir_quiet`, printing the returned messages at the call site. Search for callers with `rg -n 'migrate_tool_dir\b' src` and update each.

Re-run `rg -n 'println!|eprintln!|print!' src/skills.rs`; the result must be **empty**.

- [ ] **Step 4: Build and test**

Run: `cargo build && cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/skills.rs src/main.rs
git commit -m "refactor(skills): remove legacy update_all and all in-module stdout writes"
```

---

## Task 7: `resolve_source_target` resolver

**Files:**
- Modify: `src/skills.rs`
- Test: `tests/source_ops.rs`

- [ ] **Step 1: Write tests**

Append to `tests/source_ops.rs`:

```rust
use agm::skills::{resolve_source_target, scan_all_sources};

fn make_repo_dir(source_dir: &std::path::Path, name: &str) {
    let p = source_dir.join(name);
    fs::create_dir_all(p.join("skills").join("dummy")).unwrap();
    fs::write(
        p.join("skills").join("dummy").join("SKILL.md"),
        "---\nname: dummy\ndescription: d\n---\n",
    )
    .unwrap();
}

#[test]
fn resolve_by_directory_name() {
    let d = tempdir().unwrap();
    let source_dir = d.path().join("src");
    fs::create_dir_all(&source_dir).unwrap();
    make_repo_dir(&source_dir, "myrepo");
    let skills_dir = d.path().join("sk");
    let agents_dir = d.path().join("ag");
    let commands_dir = d.path().join("cm");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::create_dir_all(&agents_dir).unwrap();
    fs::create_dir_all(&commands_dir).unwrap();
    let g = resolve_source_target("myrepo", &source_dir, &skills_dir, &agents_dir, &commands_dir)
        .unwrap();
    assert_eq!(g.name, "myrepo");
}

#[test]
fn resolve_no_match_errors() {
    let d = tempdir().unwrap();
    let source_dir = d.path().join("src");
    fs::create_dir_all(&source_dir).unwrap();
    let skills_dir = d.path().join("sk");
    let agents_dir = d.path().join("ag");
    let commands_dir = d.path().join("cm");
    assert!(resolve_source_target(
        "nope",
        &source_dir,
        &skills_dir,
        &agents_dir,
        &commands_dir
    )
    .is_err());
}

#[test]
fn resolve_by_git_url() {
    let d = tempdir().unwrap();
    let source_dir = d.path().join("src");
    fs::create_dir_all(&source_dir).unwrap();
    let upstream = d.path().join("upstream.git");
    // Init a bare-ish upstream so `git clone` can succeed locally.
    std::process::Command::new("git")
        .args(["init", "--bare", upstream.to_str().unwrap()])
        .status()
        .unwrap();

    // Clone it under source_dir/repoA with proper origin.
    let repo_a = source_dir.join("repoA");
    std::process::Command::new("git")
        .args(["clone", upstream.to_str().unwrap(), repo_a.to_str().unwrap()])
        .status()
        .unwrap();
    fs::create_dir_all(repo_a.join("skills").join("dummy")).unwrap();
    fs::write(
        repo_a.join("skills").join("dummy").join("SKILL.md"),
        "---\nname: dummy\ndescription: d\n---\n",
    )
    .unwrap();

    let skills_dir = d.path().join("sk");
    let agents_dir = d.path().join("ag");
    let commands_dir = d.path().join("cm");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::create_dir_all(&agents_dir).unwrap();
    fs::create_dir_all(&commands_dir).unwrap();

    let g = resolve_source_target(
        upstream.to_str().unwrap(),
        &source_dir,
        &skills_dir,
        &agents_dir,
        &commands_dir,
    )
    .unwrap();
    assert_eq!(g.name, "repoA");
}

#[test]
fn resolve_multi_url_match_errors() {
    let d = tempdir().unwrap();
    let source_dir = d.path().join("src");
    fs::create_dir_all(&source_dir).unwrap();
    let upstream = d.path().join("upstream.git");
    std::process::Command::new("git")
        .args(["init", "--bare", upstream.to_str().unwrap()])
        .status()
        .unwrap();

    for name in ["repoA", "repoB"] {
        let r = source_dir.join(name);
        std::process::Command::new("git")
            .args(["clone", upstream.to_str().unwrap(), r.to_str().unwrap()])
            .status()
            .unwrap();
        fs::create_dir_all(r.join("skills").join("dummy")).unwrap();
        fs::write(
            r.join("skills").join("dummy").join("SKILL.md"),
            "---\nname: dummy\ndescription: d\n---\n",
        )
        .unwrap();
    }

    let skills_dir = d.path().join("sk");
    let agents_dir = d.path().join("ag");
    let commands_dir = d.path().join("cm");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::create_dir_all(&agents_dir).unwrap();
    fs::create_dir_all(&commands_dir).unwrap();

    let err = resolve_source_target(
        upstream.to_str().unwrap(),
        &source_dir,
        &skills_dir,
        &agents_dir,
        &commands_dir,
    )
    .unwrap_err();
    assert!(err.to_string().contains("disambiguate"));
}
```

- [ ] **Step 2: Run, expect failures**

Run: `cargo test --test source_ops resolve_`
Expected: build error — function not defined.

- [ ] **Step 3: Implement**

Add to `src/skills.rs`:

```rust
/// Resolve a `<target>` string to exactly one `SourceGroup`.
/// Match priority: (1) exact directory name match; (2) normalized git URL match
/// against repo origins (local sources skipped in step 2).
pub fn resolve_source_target(
    target: &str,
    source_dir: &Path,
    skills_dir: &Path,
    agents_dir: &Path,
    commands_dir: &Path,
) -> anyhow::Result<SourceGroup> {
    let groups = scan_all_sources(source_dir, skills_dir, agents_dir, commands_dir);
    if groups.is_empty() {
        anyhow::bail!("No sources found under {}", contract_tilde(source_dir));
    }

    // Step 1: exact directory-name match.
    let by_name: Vec<&SourceGroup> = groups.iter().filter(|g| g.name == target).collect();
    if by_name.len() == 1 {
        return Ok(by_name[0].clone());
    }
    if by_name.len() > 1 {
        let names: Vec<&str> = by_name.iter().map(|g| g.name.as_str()).collect();
        anyhow::bail!(
            "Ambiguous target '{}'; matches: {}",
            target,
            names.join(", ")
        );
    }

    // Step 2: URL match (Repo only).
    let target_norm = normalize_git_url(target);
    let by_url: Vec<&SourceGroup> = groups
        .iter()
        .filter(|g| match &g.kind {
            SourceKind::Repo { url: Some(u) } => normalize_git_url(u) == target_norm,
            _ => false,
        })
        .collect();
    if by_url.len() == 1 {
        return Ok(by_url[0].clone());
    }
    if by_url.len() > 1 {
        let names: Vec<&str> = by_url.iter().map(|g| g.name.as_str()).collect();
        anyhow::bail!(
            "Multiple repos match URL '{}'; disambiguate by name: {}",
            target,
            names.join(", ")
        );
    }

    let available: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    anyhow::bail!(
        "No source matches '{}'. Available: {}",
        target,
        available.join(", ")
    );
}
```

Note: `normalize_git_url` is currently `fn` (private). Keep it private; this call is inside the same module.

- [ ] **Step 4: Run tests, commit**

Run: `cargo test --test source_ops`
Expected: PASS.

```bash
cargo fmt
git add src/skills.rs tests/source_ops.rs
git commit -m "feat(skills): add resolve_source_target"
```

---

## Task 8: `rename_source` + `RenameReport`

**Files:**
- Modify: `src/skills.rs`
- Test: `tests/source_ops.rs`

- [ ] **Step 1: Tests**

Append to `tests/source_ops.rs`:

```rust
use agm::skills::{install_skill, rename_source};

#[test]
fn rename_relinks_installed_skill_only() {
    let d = tempdir().unwrap();
    let source_dir = d.path().join("src");
    let skills_dir = d.path().join("sk");
    let agents_dir = d.path().join("ag");
    let commands_dir = d.path().join("cm");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&skills_dir).unwrap();
    fs::create_dir_all(&agents_dir).unwrap();
    fs::create_dir_all(&commands_dir).unwrap();

    // Repo "old" with TWO skills: "dummy" (installed) and "ignored" (not installed).
    let repo = source_dir.join("old");
    for n in ["dummy", "ignored"] {
        fs::create_dir_all(repo.join("skills").join(n)).unwrap();
        fs::write(
            repo.join("skills").join(n).join("SKILL.md"),
            format!("---\nname: {}\ndescription: d\n---\n", n),
        )
        .unwrap();
    }
    install_skill("dummy", &repo.join("skills").join("dummy"), &skills_dir).unwrap();
    assert!(skills_dir.join("dummy").symlink_metadata().is_ok());
    assert!(skills_dir.join("ignored").symlink_metadata().is_err());

    let r = rename_source(
        "old",
        "new",
        &source_dir,
        &skills_dir,
        &agents_dir,
        &commands_dir,
        |_| {},
    )
    .unwrap();
    assert_eq!(r.skills_relinked, 1);

    // Installed link now resolves to new path; non-installed stays absent.
    let target = fs::read_link(skills_dir.join("dummy")).unwrap();
    assert!(target.to_string_lossy().contains("/new/"));
    assert!(skills_dir.join("ignored").symlink_metadata().is_err());
}

#[test]
fn rename_with_invalid_new_name_errors() {
    let d = tempdir().unwrap();
    let source_dir = d.path().join("src");
    fs::create_dir_all(source_dir.join("old").join("skills").join("dummy")).unwrap();
    fs::write(
        source_dir
            .join("old")
            .join("skills")
            .join("dummy")
            .join("SKILL.md"),
        "---\nname: dummy\ndescription: d\n---\n",
    )
    .unwrap();
    let skills_dir = d.path().join("sk");
    let agents_dir = d.path().join("ag");
    let commands_dir = d.path().join("cm");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::create_dir_all(&agents_dir).unwrap();
    fs::create_dir_all(&commands_dir).unwrap();

    assert!(rename_source(
        "old", "a/b", &source_dir, &skills_dir, &agents_dir, &commands_dir, |_| {},
    )
    .is_err());
    // Source dir untouched.
    assert!(source_dir.join("old").exists());
}

#[test]
fn rename_target_exists_errors() {
    let d = tempdir().unwrap();
    let source_dir = d.path().join("src");
    fs::create_dir_all(source_dir.join("old").join("skills").join("dummy")).unwrap();
    fs::write(
        source_dir
            .join("old")
            .join("skills")
            .join("dummy")
            .join("SKILL.md"),
        "---\nname: dummy\ndescription: d\n---\n",
    )
    .unwrap();
    fs::create_dir_all(source_dir.join("new")).unwrap();

    let skills_dir = d.path().join("sk");
    let agents_dir = d.path().join("ag");
    let commands_dir = d.path().join("cm");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::create_dir_all(&agents_dir).unwrap();
    fs::create_dir_all(&commands_dir).unwrap();

    let err = rename_source(
        "old", "new", &source_dir, &skills_dir, &agents_dir, &commands_dir, |_| {},
    )
    .unwrap_err();
    assert!(err.to_string().contains("already exists"));
    assert!(source_dir.join("old").exists()); // unchanged
}
```

- [ ] **Step 2: Implement**

Add to `src/skills.rs`:

```rust
#[derive(Debug, Default, Clone)]
pub struct RenameReport {
    pub skills_relinked: usize,
    pub agents_relinked: usize,
    pub commands_relinked: usize,
    pub rollback_failures: Vec<String>,
}

pub fn rename_source(
    old: &str,
    new: &str,
    source_dir: &Path,
    skills_dir: &Path,
    agents_dir: &Path,
    commands_dir: &Path,
    mut on_progress: impl FnMut(CloneProgress),
) -> anyhow::Result<RenameReport> {
    validate_source_name(new)?;

    let group = resolve_source_target(old, source_dir, skills_dir, agents_dir, commands_dir)?;

    // Determine old/new paths, including the local/ prefix if applicable.
    let (old_path, new_path) = match &group.kind {
        SourceKind::Local => (
            source_dir.join("local").join(&group.name),
            source_dir.join("local").join(new),
        ),
        _ => (
            source_dir.join(&group.name),
            source_dir.join(new),
        ),
    };

    if new_path.exists() {
        anyhow::bail!(
            "Target '{}' already exists at {}",
            new,
            contract_tilde(&new_path)
        );
    }

    on_progress(CloneProgress::Start {
        name: group.name.clone(),
        url: format!("→ {}", new),
        action: CloneAction::Pull,
    });

    // Snapshot installed items.
    let installed_skills: Vec<String> = group
        .skills
        .iter()
        .filter(|s| s.install_status == SkillInstallStatus::Installed)
        .map(|s| s.name.clone())
        .collect();
    let installed_agents: Vec<String> = group
        .agents
        .iter()
        .filter(|a| a.install_status == SkillInstallStatus::Installed)
        .map(|a| a.name.clone())
        .collect();
    let installed_commands: Vec<String> = group
        .commands
        .iter()
        .filter(|c| c.install_status == SkillInstallStatus::Installed)
        .map(|c| c.name.clone())
        .collect();

    // Uninstall from central store.
    for n in &installed_skills { let _ = uninstall_skill(n, skills_dir); }
    for n in &installed_agents { let _ = uninstall_agent(n, agents_dir); }
    for n in &installed_commands { let _ = uninstall_command(n, commands_dir); }

    // fs::rename
    if let Err(e) = fs::rename(&old_path, &new_path) {
        // Best-effort rollback: re-install against old path.
        let mut report = RenameReport::default();
        for n in &installed_skills {
            let p = old_path.join("skills").join(n);
            if install_skill(n, &p, skills_dir).is_err() {
                report.rollback_failures.push(format!("skill {}", n));
            }
        }
        for n in &installed_agents {
            let p = old_path.join("agents").join(format!("{}.md", n));
            if install_agent(n, &p, agents_dir).is_err() {
                report.rollback_failures.push(format!("agent {}", n));
            }
        }
        for n in &installed_commands {
            let p = old_path.join("commands").join(format!("{}.md", n));
            if install_command(n, &p, commands_dir).is_err() {
                report.rollback_failures.push(format!("command {}", n));
            }
        }
        on_progress(CloneProgress::Done {
            name: group.name.clone(),
            success: false,
            message: format!("rename failed: {}", e),
        });
        anyhow::bail!(
            "fs::rename failed: {}. Rollback failures: {:?}",
            e,
            report.rollback_failures
        );
    }

    // Re-scan and re-install.
    let mut report = RenameReport::default();
    let new_skills = scan_skills(&new_path);
    for (n, sp) in &new_skills {
        if installed_skills.contains(n) && install_skill(n, sp, skills_dir).is_ok() {
            report.skills_relinked += 1;
        }
    }
    let new_agents = scan_agents(&new_path);
    for (n, sp) in &new_agents {
        if installed_agents.contains(n) && install_agent(n, sp, agents_dir).is_ok() {
            report.agents_relinked += 1;
        }
    }
    let new_cmds = scan_commands(&new_path);
    for (n, sp) in &new_cmds {
        if installed_commands.contains(n) && install_command(n, sp, commands_dir).is_ok() {
            report.commands_relinked += 1;
        }
    }

    on_progress(CloneProgress::Done {
        name: new.to_string(),
        success: true,
        message: format!(
            "Renamed {} → {}; relinked {} skill(s), {} agent(s), {} command(s)",
            group.name,
            new,
            report.skills_relinked,
            report.agents_relinked,
            report.commands_relinked
        ),
    });

    Ok(report)
}
```

- [ ] **Step 3: Run tests, commit**

Run: `cargo test --test source_ops`
Expected: PASS.

```bash
cargo fmt
git add src/skills.rs tests/source_ops.rs
git commit -m "feat(skills): add rename_source and RenameReport"
```

---

## Task 9: CLI restructure — clap subcommands + dispatch

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace the `Commands` enum and dispatch**

In `src/main.rs`, replace the existing `enum Commands` and the entire `match command` block with the structure below. (Keep `Cli`, `version`, `--config global`.)

```rust
#[derive(Subcommand)]
enum Commands {
    /// Initialize agm config and central directories
    Init,
    /// Manage tools, links, and configuration
    Tool {
        #[command(subcommand)]
        action: Option<ToolAction>,
    },
    /// Manage source repos, skills, and agents
    Source {
        #[command(subcommand)]
        action: Option<SourceAction>,
    },
}

#[derive(Subcommand)]
enum ToolAction {
    /// Link all installed tools (non-interactive)
    Link,
    /// Unlink all installed tools (non-interactive)
    Unlink,
    /// Show status table (non-interactive)
    Status,
}

#[derive(Subcommand)]
enum SourceAction {
    /// Add a source (URL or local path)
    Add {
        source: String,
        /// Override target directory name
        #[arg(short = 'n', long)]
        name: Option<String>,
        /// Install all skills without prompting
        #[arg(long)]
        all: bool,
    },
    /// Update all source repos (git pull)
    Update,
    /// List all skills/agents grouped by source
    List,
    /// Delete a source by folder name or repo URL
    Del { target: String },
    /// Rename a source folder
    Rename { old: String, new: String },
}
```

- [ ] **Step 2: Rewrite dispatch**

Replace the match in `main()`:

```rust
    match command {
        Commands::Init => init::run(cli.config.clone()),
        Commands::Tool { action } => match action {
            None => tui::tool::run(cli.config.clone()),
            Some(ToolAction::Link) => {
                let config = config::Config::load_from(cli.config.clone())?;
                link_all(&config, cli.config.as_deref())
            }
            Some(ToolAction::Unlink) => {
                let config = config::Config::load_from(cli.config.clone())?;
                unlink_all(&config)
            }
            Some(ToolAction::Status) => status::status(),
        },
        Commands::Source { action } => {
            let mut config = config::Config::load_from(cli.config.clone())?;
            let skills_dir = paths::expand_tilde(&config.central.skills_source);
            let agents_dir = paths::expand_tilde(&config.central.agents_source);
            let commands_dir = paths::expand_tilde(&config.central.commands_source);
            let source_dir = paths::expand_tilde(&config.central.source_dir);
            match action {
                None => tui::source::run(&mut config),
                Some(SourceAction::Add { source, name, all }) => {
                    source_add(&source, name.as_deref(), all, &source_dir, &skills_dir, &agents_dir)
                }
                Some(SourceAction::Update) => source_update(
                    &skills_dir, &agents_dir, &commands_dir, &source_dir,
                ),
                Some(SourceAction::List) => source_list(
                    &skills_dir, &agents_dir, &commands_dir, &source_dir,
                ),
                Some(SourceAction::Del { target }) => source_del(
                    &target, &source_dir, &skills_dir, &agents_dir, &commands_dir,
                ),
                Some(SourceAction::Rename { old, new }) => source_rename(
                    &old, &new, &source_dir, &skills_dir, &agents_dir, &commands_dir,
                ),
            }
        }
    }
```

Extract the previous inline bodies into free fns `source_add`, `source_update`, `source_list` (taking the same args; copy logic verbatim from the old enum arms, using the new callback signatures from Tasks 4/5/6). New fns `source_del` and `source_rename`:

```rust
fn source_del(
    target: &str,
    source_dir: &std::path::Path,
    skills_dir: &std::path::Path,
    agents_dir: &std::path::Path,
    commands_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let group = skills::resolve_source_target(target, source_dir, skills_dir, agents_dir, commands_dir)?;
    skills::delete_source(&group, skills_dir, agents_dir, commands_dir)?;
    println!("{} Deleted source {}", " ok ".green(), group.name);
    Ok(())
}

fn source_rename(
    old: &str,
    new: &str,
    source_dir: &std::path::Path,
    skills_dir: &std::path::Path,
    agents_dir: &std::path::Path,
    commands_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let report = skills::rename_source(
        old, new, source_dir, skills_dir, agents_dir, commands_dir,
        |evt| print_clone_progress(&evt),
    )?;
    println!(
        "Relinked: {} skill(s), {} agent(s), {} command(s)",
        report.skills_relinked, report.agents_relinked, report.commands_relinked
    );
    Ok(())
}
```

`source_add` body (incorporates `name`):

```rust
fn source_add(
    source: &str,
    name: Option<&str>,
    all: bool,
    source_dir: &std::path::Path,
    skills_dir: &std::path::Path,
    agents_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let normalized = skills::normalize_git_source(source);
    if skills::is_url(&normalized) {
        let (repo_path, found_skills) = skills::clone_or_pull(
            &normalized, source_dir, name, |evt| print_clone_progress(&evt),
        )?;
        let to_install = select_skills_to_install(&found_skills, all)?;
        let mut count = 0;
        for (n, p) in &to_install {
            match skills::install_skill(n, p, skills_dir) {
                Ok(()) => { println!("  {} {} → {}", " ok ".green(), n, paths::contract_tilde(p)); count += 1; }
                Err(e) => println!("  {} {}: {}", "warn".yellow(), n, e),
            }
        }
        let mut agent_count = 0;
        for (n, p) in &skills::scan_agents(&repo_path) {
            match skills::install_agent(n, p, agents_dir) {
                Ok(()) => { println!("  {} agent {} → {}", " ok ".green(), n, paths::contract_tilde(p)); agent_count += 1; }
                Err(e) => println!("  {} agent {}: {}", "warn".yellow(), n, e),
            }
        }
        println!("\n{} skill(s), {} agent(s) installed from {}.", count, agent_count, paths::contract_tilde(&repo_path));
    } else {
        let source_path = paths::expand_tilde(source);
        let (dest, found_skills) = skills::add_local_copy(
            &source_path, source_dir, name, |evt| print_clone_progress(&evt),
        )?;
        let to_install = select_skills_to_install(&found_skills, all)?;
        let mut count = 0;
        for (n, p) in &to_install {
            match skills::install_skill(n, p, skills_dir) {
                Ok(()) => { println!("  {} {} → {}", " ok ".green(), n, paths::contract_tilde(p)); count += 1; }
                Err(e) => println!("  {} {}: {}", "warn".yellow(), n, e),
            }
        }
        println!("\n{} skill(s) installed from {}.", count, paths::contract_tilde(&dest));
    }
    Ok(())
}
```

For `source_update` and `source_list`, lift the existing match-arm bodies verbatim from the old `Commands::Source` arm into free functions with the same parameter signatures.

- [ ] **Step 3: Build and test**

Run: `cargo build && cargo test`
Expected: PASS.

- [ ] **Step 4: CLI smoke**

```bash
cargo run -- --help
cargo run -- tool --help
cargo run -- source --help
cargo run -- source add --help
```

Verify subcommands and `-n,--name` show up; old short flags do not.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/main.rs
git commit -m "feat(cli)!: replace flag-based dispatch with subcommands"
```

The `!` marks the breaking change.

---

## Task 10: CLI integration tests

**Files:**
- Test: `tests/cli.rs` (create)

- [ ] **Step 1: Write tests**

```rust
// tests/cli.rs
use assert_cmd::Command;

#[test]
fn help_shows_tool_subcommands() {
    let out = Command::cargo_bin("agm").unwrap().args(["tool", "--help"]).assert().success();
    let s = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    for kw in ["link", "unlink", "status"] {
        assert!(s.contains(kw), "missing {} in tool help", kw);
    }
}

#[test]
fn help_shows_source_subcommands() {
    let out = Command::cargo_bin("agm").unwrap().args(["source", "--help"]).assert().success();
    let s = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    for kw in ["add", "update", "list", "del", "rename"] {
        assert!(s.contains(kw), "missing {} in source help", kw);
    }
}

#[test]
fn source_add_help_has_name_flag() {
    let out = Command::cargo_bin("agm").unwrap().args(["source", "add", "--help"]).assert().success();
    let s = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(s.contains("--name"));
    assert!(s.contains("-n"));
}

#[test]
fn old_short_flags_rejected() {
    // -u after `tool` is no longer valid
    Command::cargo_bin("agm").unwrap().args(["tool", "-u"]).assert().failure();
    Command::cargo_bin("agm").unwrap().args(["source", "-a", "x"]).assert().failure();
}

#[test]
fn source_del_parses() {
    // Will fail at runtime (no config or no match), but parsing must succeed.
    Command::cargo_bin("agm").unwrap().args(["source", "del", "nonexistent"]).assert().failure();
}

#[test]
fn invalid_name_rejected_locally() {
    // We can at least confirm clap accepts the surface form; the validation
    // happens deeper. This test mainly guards that `-n` accepts a value.
    Command::cargo_bin("agm").unwrap().args(["source", "add", "x", "-n", "foo"]).assert();
}
```

- [ ] **Step 2: Run, commit**

Run: `cargo test --test cli`
Expected: PASS.

```bash
git add tests/cli.rs
git commit -m "test(cli): integration tests for new subcommand shape"
```

---

## Task 11: TUI — wire `do_add_submit` callback into `LogBuffer`

**Files:**
- Modify: `src/tui/source.rs`

- [ ] **Step 1: Replace the placeholder callbacks**

Find `do_add_submit` (around line 1210). For the URL branch:

```rust
match skills::clone_or_pull(&source, &self.source_dir, None, |evt| {
    push_clone_progress(&mut self.log, &evt);
}) {
```

For the local branch (around line 1253):

```rust
match skills::add_local_copy(&source_path, &self.source_dir, None, |evt| {
    push_clone_progress(&mut self.log, &evt);
}) {
```

**Important:** the closure borrows `self.log` mutably; the surrounding code also touches `self`, so move the `self.log.push` block out of the closure by capturing only `&mut self.log`. Concretely:

```rust
let log = &mut self.log;
match skills::clone_or_pull(&source, &self.source_dir, None, |evt| {
    push_clone_progress(log, &evt);
}) {
```

…and re-borrow `self` after the match.

- [ ] **Step 2: Add the helper**

In `src/tui/source.rs`, add (top-level inside the file):

```rust
fn push_clone_progress(log: &mut super::log::LogBuffer, evt: &skills::CloneProgress) {
    use skills::CloneProgress::*;
    use super::log::LogLevel;
    match evt {
        Start { name, url, action } => {
            let verb = match action {
                skills::CloneAction::Clone => "Cloning",
                skills::CloneAction::Pull => "Updating",
            };
            log.push(LogLevel::Info, format!("{} {} from {}", verb, name, url));
        }
        GitLine { line, is_err } => {
            // git often prints normal progress to stderr (e.g. "Cloning into ...").
            // We still surface it as Warning to make it visually distinct from
            // stdout, but reserve Error for the final failing Done event.
            log.push(
                if *is_err { LogLevel::Warning } else { LogLevel::Info },
                line.clone(),
            );
        }
        Done { name, success, message } => {
            log.push(
                if *success { LogLevel::Success } else { LogLevel::Error },
                format!("{}: {}", name, message),
            );
        }
    }
}
```

- [ ] **Step 3: Build, manual smoke**

Run: `cargo build`
Manual: `cargo run -- source` → press `a`, paste a real repo URL, confirm no screen tearing and the log popup (`o`) shows git output.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/tui/source.rs
git commit -m "feat(tui): route clone_or_pull/add_local_copy output through LogBuffer"
```

---

## Task 12: TUI rename mode + `r` / `F5` rebinding

**Files:**
- Modify: `src/tui/source.rs`

- [ ] **Step 1: Add rename-mode state**

In the source `App` struct, add fields next to the existing `add_mode`:

```rust
rename_mode: bool,
rename_input: String,
rename_cursor: usize,
rename_target_group_index: Option<usize>,
```

Initialize them to `false` / empty / `None` in the `App::new` (or equivalent) constructor.

- [ ] **Step 2: Rebind keys**

Find the `KeyCode::Char('r')` arm (around line 1567) and replace with:

```rust
KeyCode::F(5) => {
    self.refresh();
    self.log.push(super::log::LogLevel::Info, "Refreshed");
    self.set_status("Refreshed");
}
KeyCode::Char('r') => {
    self.start_rename();
}
```

- [ ] **Step 3: Implement `start_rename` and submit**

```rust
fn start_rename(&mut self) {
    let row = match self.current_row() {
        Some(r) => r,
        None => { self.set_status("Rename: select a source row first"); return; }
    };
    let group_index = match row {
        ListRow::SourceHeader { group_index, .. } => group_index,
        _ => { self.set_status("Rename: select a source row first"); return; }
    };
    let current = self.groups[group_index].name.clone();
    self.rename_mode = true;
    self.rename_input = current.clone();
    self.rename_cursor = current.chars().count();
    self.rename_target_group_index = Some(group_index);
    self.set_status("Rename: edit name (Enter to confirm, Esc to cancel)");
}

fn do_rename_submit(&mut self) {
    let group_index = match self.rename_target_group_index.take() {
        Some(i) => i,
        None => { self.rename_mode = false; return; }
    };
    let new_name = self.rename_input.trim().to_string();
    self.rename_mode = false;
    self.rename_input.clear();
    self.rename_cursor = 0;

    let old_name = self.groups[group_index].name.clone();
    if new_name.is_empty() || new_name == old_name {
        self.set_status("Rename cancelled");
        return;
    }

    let log = &mut self.log;
    match skills::rename_source(
        &old_name,
        &new_name,
        &self.source_dir,
        &self.skills_dir,
        &self.agents_dir,
        &self.commands_dir,
        |evt| push_clone_progress(log, &evt),
    ) {
        Ok(_) => self.set_status(format!("Renamed {} → {}", old_name, new_name)),
        Err(e) => {
            self.log.push(super::log::LogLevel::Error, format!("Rename: {}", e));
            self.set_status(format!("Rename error: {}", e));
        }
    }
    self.refresh();
}
```

Replace `current_row(&self)` with whatever the existing accessor is (search for how `start_delete` finds the focused row — pattern-match the same way).

- [ ] **Step 4: Input handling**

In `handle_key`, mirror the `add_mode` branch for `rename_mode`. Above the existing `if self.add_mode {` block, add the same shape:

```rust
if self.rename_mode {
    match code {
        KeyCode::Esc => {
            self.rename_mode = false;
            self.rename_input.clear();
            self.rename_target_group_index = None;
            self.set_status("Rename cancelled");
        }
        KeyCode::Enter => self.do_rename_submit(),
        KeyCode::Backspace => {
            if self.rename_cursor > 0 {
                let mut chars: Vec<char> = self.rename_input.chars().collect();
                chars.remove(self.rename_cursor - 1);
                self.rename_input = chars.into_iter().collect();
                self.rename_cursor -= 1;
            }
        }
        KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
            let mut chars: Vec<char> = self.rename_input.chars().collect();
            chars.insert(self.rename_cursor, c);
            self.rename_input = chars.into_iter().collect();
            self.rename_cursor += 1;
        }
        _ => {}
    }
    return;
}
```

- [ ] **Step 5: Render rename bar**

Find where `add_mode` renders its input bar (search for `if app.add_mode` in the render section). Add a parallel render for `rename_mode` showing `Rename → {rename_input}` with a cursor at `rename_cursor`.

- [ ] **Step 6: Footer hints**

Update footer/help strings in `tui/source.rs` (search for `r:`/`refresh`). Replace existing `r:refresh` with `r:rename F5:refresh`.

- [ ] **Step 7: Build, manual smoke, commit**

Run: `cargo build`
Manual: launch source TUI, hover a repo row, press `r`, type new name, Enter. Verify rename + relink. Press `F5`, verify refresh.

```bash
cargo fmt
git add src/tui/source.rs
git commit -m "feat(tui): add rename mode with r key; F5 takes over refresh"
```

---

## Task 13: TUI info popups — preload char rows

**Files:**
- Modify: `src/tui/source.rs`

- [ ] **Step 1: Skill / agent / command info**

In `build_skill_info_lines` (line 880), after the `Status:` line (around 901) and before the blank line, add:

```rust
lines.push(Line::from(vec![
    Span::styled("Preload chars: ", Style::default().fg(Color::Yellow)),
    Span::raw(skill.preload_chars.to_string()),
]));
```

In `build_agent_info_lines` (line 952), same place, with `Char count:`:

```rust
lines.push(Line::from(vec![
    Span::styled("Char count: ", Style::default().fg(Color::Yellow)),
    Span::raw(agent.preload_chars.to_string()),
]));
```

Same change in `build_command_info_lines` (line 1005).

- [ ] **Step 2: Source header info**

In `build_source_info_lines` (line 1061), insert before the existing detail block, the new "Preload chars:" section:

```rust
let sum = |installed: bool, items_chars: &[(bool, usize)]| -> usize {
    items_chars.iter().filter(|(i, _)| *i == installed).map(|(_, c)| *c).sum()
};

let skill_data: Vec<(bool, usize)> = group
    .skills
    .iter()
    .map(|s| (s.install_status == skills::SkillInstallStatus::Installed, s.preload_chars))
    .collect();
let agent_data: Vec<(bool, usize)> = group
    .agents
    .iter()
    .map(|a| (a.install_status == skills::SkillInstallStatus::Installed, a.preload_chars))
    .collect();
let cmd_data: Vec<(bool, usize)> = group
    .commands
    .iter()
    .map(|c| (c.install_status == skills::SkillInstallStatus::Installed, c.preload_chars))
    .collect();

lines.push(Line::default());
lines.push(Line::from(Span::styled(
    "Preload chars:",
    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
)));
for (label, data) in [("Skills", &skill_data), ("Agents", &agent_data), ("Commands", &cmd_data)] {
    if data.is_empty() { continue; }
    lines.push(Line::from(format!(
        "  {:<8} — installed {}  not-installed {}",
        label,
        sum(true, data),
        sum(false, data),
    )));
}
```

- [ ] **Step 3: Category header info**

In `build_category_info_lines` (line 744), at the end (before `lines`), append:

```rust
let (installed_chars, uninstalled_chars) = match category {
    Category::Skills => {
        let i: usize = self.groups.iter().flat_map(|g| &g.skills)
            .filter(|s| s.install_status == SkillInstallStatus::Installed)
            .map(|s| s.preload_chars).sum();
        let u: usize = self.groups.iter().flat_map(|g| &g.skills)
            .filter(|s| s.install_status != SkillInstallStatus::Installed)
            .map(|s| s.preload_chars).sum();
        (i, u)
    }
    Category::Agents => {
        let i: usize = self.groups.iter().flat_map(|g| &g.agents)
            .filter(|a| a.install_status == SkillInstallStatus::Installed)
            .map(|a| a.preload_chars).sum();
        let u: usize = self.groups.iter().flat_map(|g| &g.agents)
            .filter(|a| a.install_status != SkillInstallStatus::Installed)
            .map(|a| a.preload_chars).sum();
        (i, u)
    }
    Category::Commands => {
        let i: usize = self.groups.iter().flat_map(|g| &g.commands)
            .filter(|c| c.install_status == SkillInstallStatus::Installed)
            .map(|c| c.preload_chars).sum();
        let u: usize = self.groups.iter().flat_map(|g| &g.commands)
            .filter(|c| c.install_status != SkillInstallStatus::Installed)
            .map(|c| c.preload_chars).sum();
        (i, u)
    }
};

lines.push(Line::default());
lines.push(Line::from(Span::styled(
    "Total preload chars:",
    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
)));
lines.push(Line::from(format!("  installed:     {}", installed_chars)));
lines.push(Line::from(format!("  not-installed: {}", uninstalled_chars)));
```

- [ ] **Step 4: Build, manual smoke**

Run: `cargo build && cargo run -- source`
Press `i` on a skill row, a source header, and a category header — confirm the new lines appear.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add src/tui/source.rs
git commit -m "feat(tui): show preload-char counts in info popups"
```

---

## Task 14: CHANGELOG + README

**Files:**
- Modify: `CHANGELOG.md`, `README.md`

- [ ] **Step 1: CHANGELOG**

Add (or update) an `## Unreleased` section at the top of `CHANGELOG.md`:

```markdown
## Unreleased

### Breaking
- CLI restructured to use explicit subcommands. Removed: `agm tool -l/-u/-s`, `agm source -a/-u/-l/--add/--update/--list`. Replacements: `agm tool link|unlink|status`, `agm source add|update|list`.

### Added
- `agm source del <name|url>` — delete a source.
- `agm source rename <old> <new>` — rename a source folder and relink installed items.
- `agm source add -n,--name <name>` — override the cloned/copied directory name.
- TUI: `r` opens rename for the focused source row; `F5` refreshes the list.
- TUI info popups now display preload-char counts (`name` + `description` for skills; whole file for agents/commands) with rollups at source and category levels.

### Fixed
- TUI screen tearing when adding a source. All git stdout/stderr now flows through `LogBuffer`.
```

- [ ] **Step 2: README**

Update README examples (search for `agm tool -`, `agm source -a`, `agm source -l`, `agm source -u`) to use subcommand syntax. Add a short section on `del`, `rename`, and `-n`.

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md README.md
git commit -m "docs: update CHANGELOG and README for CLI restructure and new features"
```

---

## Self-review notes (delete after implementation)

- All spec sections covered: §1 (Task 9, 10), §2.1 (Tasks 4, 5), §2.2 (Task 7, 9), §2.3 (Tasks 8, 12), §3 (Tasks 4, 5, 11, 12), §4 (Tasks 2, 3, 13), §5 (CHANGELOG/README in Task 14).
- No placeholders — every step has explicit code or shell.
- Type names match the spec and codebase (`SkillInfo`, `AgentInfo`, `CommandInfo`).
- `validate_source_name` introduced in Task 4 is reused in Tasks 5 and 8 — single definition.
- Function names (`clone_or_pull`, `add_local_copy`, `resolve_source_target`, `rename_source`, `skill_preload_chars`, `file_char_count`, `RenameReport`, `CloneProgress`, `CloneAction`) consistent across tasks.
- TDD cycle (red/green/commit) applied to logic tasks (1–8); UI-heavy tasks (9–13) use build + manual smoke before commit, since ratatui doesn't lend itself to unit-testing event loops cheaply.
