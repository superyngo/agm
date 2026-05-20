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

use agm::skills::validate_source_name;

use agm::skills::resolve_source_target;

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
    let g = resolve_source_target(
        "myrepo",
        &source_dir,
        &skills_dir,
        &agents_dir,
        &commands_dir,
    )
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
    assert!(
        resolve_source_target("nope", &source_dir, &skills_dir, &agents_dir, &commands_dir)
            .is_err()
    );
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
        .args([
            "clone",
            upstream.to_str().unwrap(),
            repo_a.to_str().unwrap(),
        ])
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
#[ignore] // requires network to fail-resolve the broken hostname
fn clone_or_pull_routes_errors_through_callback_not_stdout() {
    use agm::skills::{clone_or_pull, CloneProgress};
    use std::sync::{Arc, Mutex};

    let d = tempdir().unwrap();
    let source_dir = d.path().join("src");

    // Deliberately broken URL — git will fail. We assert:
    //   (a) function returns Err
    //   (b) at least one GitLine { is_err: true } was emitted
    //   (c) a failing Done event was received
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
        evts.iter()
            .any(|e| matches!(e, CloneProgress::GitLine { is_err: true, .. })),
        "expected at least one stderr GitLine event, got: {:?}",
        evts
    );
    assert!(
        evts.iter()
            .any(|e| matches!(e, CloneProgress::Done { success: false, .. })),
        "expected a failing Done event"
    );
}

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
    assert!(target.components().any(|c| c.as_os_str() == "new"));
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
        "old",
        "a/b",
        &source_dir,
        &skills_dir,
        &agents_dir,
        &commands_dir,
        |_| {},
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
        "old",
        "new",
        &source_dir,
        &skills_dir,
        &agents_dir,
        &commands_dir,
        |_| {},
    )
    .unwrap_err();
    assert!(err.to_string().contains("already exists"));
    assert!(source_dir.join("old").exists()); // unchanged
}
