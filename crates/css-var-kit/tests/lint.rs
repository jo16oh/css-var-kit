mod common;

use common::cvk;
use predicates::prelude::PredicateBooleanExt;
use std::fs;

#[test]
fn reports_undefined_variables() {
    cvk()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--spacing-md"))
        .stderr(predicates::str::contains("--border-color"))
        .stderr(predicates::str::contains("--radius-lg"));
}

#[test]
fn exclude_files_suppresses_lint_for_matched_files() {
    let tmp = common::copy_fixture_to_tempdir("default");
    fs::write(
        tmp.path().join("cvk.json"),
        r#"{"excludeFiles": ["components/button.css"]}"#,
    )
    .unwrap();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(tmp.path());
    // card.css still has errors (--radius-lg, --spacing-md), so lint exits non-zero.
    // button.css is excluded, so --border-color (defined only there) must not appear.
    cmd.args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--radius-lg"))
        .stderr(predicates::str::contains("--border-color").not());
}
