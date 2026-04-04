mod common;

use common::FIXTURES;

fn cvk_undefined() -> assert_cmd::Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/undefined-variable"));
    cmd
}

#[test]
fn diagnostic_points_to_variable_name() {
    cvk_undefined()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "single.css:1:17 no-undefined-variable-use",
        ))
        .stderr(predicates::str::contains(concat!(
            "  > 1 │ .a { color: var(--undefined); }\n",
            "      │                 ^^^^^^^^^^^",
        )));
}

#[test]
fn diagnostic_points_to_each_variable_in_multiple() {
    cvk_undefined()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "multiple.css:1:22 no-undefined-variable-use",
        ))
        .stderr(predicates::str::contains(concat!(
            "  > 1 │ .a { background: var(--a) var(--b); }\n",
            "      │                      ^^^",
        )))
        .stderr(predicates::str::contains(
            "multiple.css:1:31 no-undefined-variable-use",
        ))
        .stderr(predicates::str::contains(concat!(
            "  > 1 │ .a { background: var(--a) var(--b); }\n",
            "      │                               ^^^",
        )));
}

#[test]
fn diagnostic_points_to_nested_var() {
    cvk_undefined()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "nested.css:2:17 no-undefined-variable-use",
        ))
        .stderr(predicates::str::contains(concat!(
            "  > 2 │ .a { color: var(--primary, var(--fb)); }\n",
            "      │                 ^^^^^^^^^",
        )));
}
