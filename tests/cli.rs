use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_shows_tool_subcommand() {
    Command::cargo_bin("agm")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("tool"));
}

#[test]
fn test_help_shows_source_subcommand() {
    Command::cargo_bin("agm")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("source"));
}

#[test]
fn test_tool_help_shows_flags() {
    Command::cargo_bin("agm")
        .unwrap()
        .args(["tool", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--link"))
        .stdout(predicate::str::contains("--unlink"))
        .stdout(predicate::str::contains("--status"));
}

#[test]
fn test_tool_mutually_exclusive_flags() {
    Command::cargo_bin("agm")
        .unwrap()
        .args(["tool", "--link", "--unlink"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Only one of"));
}

#[test]
fn test_version_flag() {
    Command::cargo_bin("agm")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("agm"));
}

#[test]
fn test_old_commands_removed() {
    for cmd in &[
        "link", "unlink", "status", "config", "prompt", "auth", "mcp",
    ] {
        Command::cargo_bin("agm")
            .unwrap()
            .arg(cmd)
            .assert()
            .failure();
    }
}
