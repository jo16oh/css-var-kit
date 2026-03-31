mod common;

use common::cvk;

#[test]
fn rule_off_disables_rule() {
    cvk()
        .args([
            "lint",
            "--rule",
            "no-undefined-variable-use=off",
            "--rule",
            "no-variable-type-mismatch=off",
        ])
        .assert()
        .success()
        .stderr(predicates::str::is_empty());
}

#[test]
fn unknown_rule_name_errors() {
    cvk()
        .args(["lint", "--rule", "bad-rule=on"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown rule 'bad-rule'"));
}

#[test]
fn invalid_format_errors() {
    cvk()
        .args(["lint", "--rule", "no-value"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("expected format NAME=VALUE"));
}

#[test]
fn invalid_toggle_value_errors() {
    cvk()
        .args(["lint", "--rule", "no-undefined-variable-use=yes"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "expected 'error', 'warn', 'on', or 'off'",
        ));
}

#[test]
fn enforce_variable_use_json() {
    cvk()
        .args([
            "lint",
            "--rule",
            r#"enforce-variable-use={"types":["color"]}"#,
        ])
        .assert()
        .failure(); // fails because no-undefined-variable-use is still on
}

#[test]
fn enforce_variable_use_off() {
    cvk()
        .args([
            "lint",
            "--rule",
            "enforce-variable-use=off",
            "--rule",
            "no-undefined-variable-use=off",
            "--rule",
            "no-variable-type-mismatch=off",
        ])
        .assert()
        .success();
}

#[test]
fn enforce_variable_use_invalid_json_errors() {
    cvk()
        .args(["lint", "--rule", "enforce-variable-use={bad json}"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("invalid --rule value"));
}

#[test]
fn enforce_variable_use_invalid_value_errors() {
    cvk()
        .args(["lint", "--rule", "enforce-variable-use=yes"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "expected 'error', 'warn', 'on', 'off', or a JSON object",
        ));
}

#[test]
fn missing_dependency_no_undefined_variable_use() {
    cvk()
        .args(["lint", "--rule", "no-undefined-variable-use=off"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "rule 'no-variable-type-mismatch' requires 'no-undefined-variable-use' to be enabled",
        ));
}

#[test]
fn missing_dependency_no_inconsistent_variable_definition() {
    cvk()
        .args([
            "lint",
            "--rule",
            "no-inconsistent-variable-definition=off",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "rule 'no-variable-type-mismatch' requires 'no-inconsistent-variable-definition' to be enabled",
        ));
}

#[test]
fn multiple_rule_overrides() {
    cvk()
        .args([
            "lint",
            "--rule",
            "no-undefined-variable-use=off",
            "--rule",
            "no-variable-type-mismatch=off",
            "--rule",
            "no-inconsistent-variable-definition=off",
        ])
        .assert()
        .success();
}
