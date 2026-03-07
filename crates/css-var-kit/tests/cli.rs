use assert_cmd::Command;

fn cvk() -> Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"));
    cmd
}

#[test]
fn lint_default_reports_undefined_variables() {
    cvk()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--spacing-md"))
        .stderr(predicates::str::contains("--border-color"))
        .stderr(predicates::str::contains("--radius-lg"));
}

#[test]
fn lint_rule_off_disables_rule() {
    cvk()
        .args(["lint", "--rule", "no-undefined-variable-use=off"])
        .assert()
        .success()
        .stderr(predicates::str::is_empty());
}

#[test]
fn lint_rule_unknown_name_errors() {
    cvk()
        .args(["lint", "--rule", "bad-rule=on"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown rule 'bad-rule'"));
}

#[test]
fn lint_rule_invalid_format_errors() {
    cvk()
        .args(["lint", "--rule", "no-value"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("expected format NAME=VALUE"));
}

#[test]
fn lint_rule_invalid_toggle_value_errors() {
    cvk()
        .args(["lint", "--rule", "no-undefined-variable-use=yes"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("expected 'on' or 'off'"));
}

#[test]
fn lint_rule_enforce_variable_use_json() {
    // With JSON config, lint should still run (enforce-variable-use doesn't produce
    // diagnostics yet, but it shouldn't error on valid JSON)
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
fn lint_rule_enforce_variable_use_off() {
    // enforce-variable-use=off with no-undefined-variable-use=off should succeed
    cvk()
        .args([
            "lint",
            "--rule",
            "enforce-variable-use=off",
            "--rule",
            "no-undefined-variable-use=off",
        ])
        .assert()
        .success();
}

#[test]
fn lint_rule_enforce_variable_use_invalid_json_errors() {
    cvk()
        .args(["lint", "--rule", "enforce-variable-use={bad json}"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("invalid --rule value"));
}

#[test]
fn lint_rule_enforce_variable_use_invalid_value_errors() {
    cvk()
        .args(["lint", "--rule", "enforce-variable-use=yes"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "expected 'on', 'off', or a JSON object",
        ));
}

#[test]
fn lint_multiple_rule_overrides() {
    cvk()
        .args([
            "lint",
            "--rule",
            "no-undefined-variable-use=off",
            "--rule",
            "no-compound-value-in-definition=off",
            "--rule",
            "no-type-mismatch=off",
        ])
        .assert()
        .success();
}
