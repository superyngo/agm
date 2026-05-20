// tests/source_ops.rs
use agm::skills::{CloneAction, CloneProgress};

#[test]
fn clone_progress_variants_constructible() {
    let _ = CloneProgress::Start {
        name: "r".into(),
        url: "u".into(),
        action: CloneAction::Clone,
    };
    let _ = CloneProgress::GitLine {
        line: "x".into(),
        is_err: false,
    };
    let _ = CloneProgress::Done {
        name: "r".into(),
        success: true,
        message: "ok".into(),
    };
}

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
    assert_eq!(
        skill_preload_chars(&skill),
        "foo".chars().count() + "hello".chars().count()
    );
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
