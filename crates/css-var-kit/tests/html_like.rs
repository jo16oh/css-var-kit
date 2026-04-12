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
    // ErrorComponent.vue の --undefined-color がエラーとして報告される
    cvk_html_like()
        .args(["lint"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--undefined-color"));
}

#[test]
fn valid_vue_svelte_astro_files_pass() {
    // ErrorComponent.vue と LessComponent.vue を除いたフィクスチャで検証する
    // ここでは Button.vue / Card.svelte / Page.astro が valid であることを
    // tokens.css の変数定義と合わせて確認する。
    // ErrorComponent.vue によるエラーが stderr に出ることを確認済みなので、
    // 逆にエラーがないケースのテストとして Button.vue の内容に関するエラーが
    // 出ないことを確認する。
    cvk_html_like()
        .args(["lint"])
        .assert()
        .stderr(predicates::str::contains("--color-primary").not())
        .stderr(predicates::str::contains("--spacing-md").not());
}

#[test]
fn less_lang_style_block_is_skipped() {
    // LessComponent.vue の less ブロックはパースされないため
    // @box-color に関するエラーは出ない
    cvk_html_like()
        .args(["lint"])
        .assert()
        .stderr(predicates::str::contains("@box-color").not());
}
