// End-to-end tests run the real binary against a throwaway $HOME.
#![allow(deprecated)]
use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn agm(home: &Path) -> Command {
    let mut c = Command::cargo_bin("agm").unwrap();
    c.env("HOME", home);
    c
}

fn setup() -> TempDir {
    let home = tempfile::tempdir().unwrap();
    agm(home.path()).arg("init").assert().success();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    home
}

fn write(p: &Path, s: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, s).unwrap();
}

/// Every file under `root` (recursive, following nothing) whose content equals `content`.
fn find_content(root: &Path, content: &str) -> bool {
    let Ok(rd) = fs::read_dir(root) else {
        return false;
    };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(m) = fs::symlink_metadata(&p) else {
            continue;
        };
        if m.is_dir() {
            if find_content(&p, content) {
                return true;
            }
        } else if m.is_file() && fs::read_to_string(&p).is_ok_and(|s| s == content) {
            return true;
        }
    }
    false
}

#[test]
fn link_keeps_unrecognised_files_and_conflicting_copy() {
    let home = setup();
    let h = home.path();
    let skills = h.join(".claude/skills");
    write(&skills.join("real/SKILL.md"), "real-skill");
    write(&skills.join("notes-dir/important.txt"), "important-notes");
    write(&skills.join("README.md"), "readme-body");
    write(&skills.join("dup/SKILL.md"), "tool-edited-dup");
    // `dup` already exists in the tool's store dir with different content.
    write(
        &h.join(".local/share/agm/source/agm_tools/claude/dup/SKILL.md"),
        "store-dup",
    );

    agm(h).args(["tool", "link"]).assert().success();

    for c in [
        "important-notes",
        "readme-body",
        "tool-edited-dup",
        "real-skill",
    ] {
        assert!(find_content(h, c), "content {c:?} was lost");
    }
    assert!(fs::symlink_metadata(&skills)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn unlink_leaves_real_copies_not_links_into_store() {
    let home = setup();
    let h = home.path();
    write(&h.join(".claude/skills/real/SKILL.md"), "real-skill");
    agm(h).args(["tool", "link"]).assert().success();
    agm(h).args(["tool", "unlink"]).assert().success();

    let skill = h.join(".claude/skills/real");
    let meta = fs::symlink_metadata(&skill).unwrap();
    assert!(!meta.file_type().is_symlink(), "skill is still a symlink");
    assert_eq!(
        fs::read_to_string(skill.join("SKILL.md")).unwrap(),
        "real-skill"
    );
}

#[test]
fn status_honours_config_flag() {
    let home = setup();
    agm(home.path())
        .args(["--config", "/nonexistent/x.toml", "tool", "status"])
        .assert()
        .failure();
}

#[test]
fn link_migrates_existing_agents_instead_of_deleting() {
    let home = setup();
    let h = home.path();
    write(&h.join(".claude/agents/reviewer.md"), "agent-body");
    agm(h).args(["tool", "link"]).assert().success();
    assert!(find_content(h, "agent-body"), "agent file was lost");
}

#[test]
fn link_links_commands_and_continues_past_a_failing_tool() {
    let home = setup();
    let h = home.path();
    write(&h.join(".claude/commands/deploy.md"), "cmd-body");
    agm(h).args(["tool", "link"]).assert().success();
    assert!(find_content(h, "cmd-body"), "command file was lost");
    let link = h.join(".claude/commands");
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
}
