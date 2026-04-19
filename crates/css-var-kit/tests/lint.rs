mod common;

use common::{FIXTURES, cvk};
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
fn include_negation_excludes_file_from_lint() {
    let tmp = common::copy_fixture_to_tempdir("default");
    fs::write(
        tmp.path().join("cvk.json"),
        r#"{"include": ["!components/button.css"]}"#,
    )
    .unwrap();

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(tmp.path());
    // card.css still has errors (--radius-lg, --spacing-md), so lint exits non-zero.
    // button.css is excluded via negation, so --border-color must not appear.
    cmd.args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--radius-lg"))
        .stderr(predicates::str::contains("--border-color").not());
}

#[test]
fn path_argument_limits_diagnostics_to_specified_file() {
    cvk()
        .args(["lint", "components/button.css"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--spacing-md"))
        .stderr(predicates::str::contains("--border-color"))
        .stderr(predicates::str::contains("--radius-lg").not());
}

#[test]
fn path_argument_multiple_files() {
    cvk()
        .args(["lint", "components/button.css", "components/card.css"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--border-color"))
        .stderr(predicates::str::contains("--radius-lg"));
}

#[test]
fn path_argument_no_errors_exits_zero() {
    cvk().args(["lint", "variables.css"]).assert().success();
}

#[test]
fn path_argument_resolves_relative_to_cwd() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/default/components"));
    cmd.args(["lint", "button.css"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--border-color"))
        .stderr(predicates::str::contains("--radius-lg").not());
}
