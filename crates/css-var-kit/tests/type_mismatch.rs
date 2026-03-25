mod common;

use common::FIXTURES;

fn cvk_type_mismatch() -> assert_cmd::Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/type-mismatch"));
    cmd
}

#[test]
fn reports_type_mismatch() {
    cvk_type_mismatch()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "Type mismatch: resolved value of `var(--size)` is not valid for property `color`",
        ))
        .stderr(predicates::str::contains(
            "Type mismatch: resolved value of `var(--color)` is not valid for property `width`",
        ));
}

#[test]
fn reports_nested_var_mismatch() {
    cvk_type_mismatch()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "Type mismatch: resolved value of `var(--alias-size)` is not valid for property `color`",
        ));
}

#[test]
fn no_report_when_rule_is_off() {
    cvk_type_mismatch()
        .args(["lint", "--rule", "no-variable-type-mismatch=off"])
        .assert()
        .success();
}
