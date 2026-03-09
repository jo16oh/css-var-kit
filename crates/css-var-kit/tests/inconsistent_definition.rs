mod common;

use common::FIXTURES;
use predicates::prelude::PredicateBooleanExt;

fn cvk_inconsistent() -> assert_cmd::Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/inconsistent-definition"));
    cmd
}

#[test]
fn reports_inconsistent_definition() {
    cvk_inconsistent()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "inconsistent variable definition: `--size` has value `300ms` which conflicts with expected type <length",
        ));
}

#[test]
fn consistent_definitions_no_warning() {
    cvk_inconsistent()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--color").not());
}

#[test]
fn no_report_when_rule_is_off() {
    cvk_inconsistent()
        .args(["lint", "--rule", "no-inconsistent-variable-definition=off"])
        .assert()
        .success();
}
