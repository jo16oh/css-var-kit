mod common;

use common::cvk;

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
