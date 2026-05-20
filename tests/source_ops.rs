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
