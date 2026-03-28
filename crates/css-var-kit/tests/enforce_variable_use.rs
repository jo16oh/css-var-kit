mod common;

use common::FIXTURES;
use predicates::prelude::PredicateBooleanExt;

fn cvk_enforce() -> assert_cmd::Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/enforce-variable-use"));
    cmd
}

#[test]
fn reports_literal_color() {
    cvk_enforce()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "use a CSS variable instead of the literal color `red`",
        ));
}

#[test]
fn reports_literal_color_in_shorthand() {
    cvk_enforce()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "use a CSS variable instead of the literal color `blue`",
        ));
}

#[test]
fn skips_variable_use() {
    cvk_enforce()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("literal color `var(--bg)`").not());
}

#[test]
fn skips_inherit() {
    cvk_enforce()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("literal color `inherit`").not());
}

#[test]
fn skips_custom_property_definition() {
    cvk_enforce()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--custom").not());
}

#[test]
fn cvk_ignore_suppresses() {
    cvk_enforce()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("green").not());
}

#[test]
fn rule_off_disables() {
    cvk_enforce()
        .args(["lint", "--rule", "enforce-variable-use=off"])
        .assert()
        .success();
}
