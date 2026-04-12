mod common;

use assert_cmd::Command;
use common::FIXTURES;
use predicates::prelude::PredicateBooleanExt;

fn cvk_html_like() -> Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/html-like"));
    cmd
}

#[test]
fn vue_svelte_astro_files_are_linted() {
    cvk_html_like()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--undefined-color"));
}

#[test]
fn valid_vue_svelte_astro_files_pass() {
    cvk_html_like()
        .args(["lint"])
        .assert()
        .stderr(predicates::str::contains("--color-primary").not())
        .stderr(predicates::str::contains("--spacing-md").not());
}

#[test]
fn less_lang_style_block_is_skipped() {
    cvk_html_like()
        .args(["lint"])
        .assert()
        .stderr(predicates::str::contains("@box-color").not());
}
