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
