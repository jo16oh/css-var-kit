mod common;

use common::FIXTURES;

#[test]
fn root_dir_is_relative_to_config_file() {
    // config-base-test/configs/cvk.json has rootDir: "../css"
    // This should resolve to config-base-test/css/ (relative to the config file),
    // which contains only style.css with no undefined variables.
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/config-base-test"));
    cmd.args(["lint", "-c", "configs/cvk.json"])
        .assert()
        .success();
}

#[test]
fn config_with_trailing_commas_is_discovered_and_parsed() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/trailing-comma"));
    cmd.arg("lint")
        .assert()
        .success()
        .stderr(predicates::str::is_empty());
}

#[test]
fn config_with_unterminated_block_comment_is_rejected() {
    // unterminated-comment/cvk.json disables the rules before an unclosed `/*`,
    // so lint would pass if the dangling comment were silently accepted.
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/unterminated-comment"));
    cmd.arg("lint")
        .assert()
        .failure()
        .stderr(predicates::str::contains("failed to parse"));
}

#[test]
fn config_with_utf8_bom_is_parsed() {
    // bom/cvk.json starts with a UTF-8 BOM and disables no-undefined-variable-use,
    // so style.css (which uses an undefined variable) should pass.
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/bom"));
    cmd.arg("lint").assert().success();
}

#[test]
fn unreadable_config_is_reported_instead_of_falling_back() {
    // unreadable-config/cvk.json is not valid UTF-8. It must be reported as an error
    // rather than silently skipped in favor of cvk.jsonc, which disables the rules.
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/unreadable-config"));
    cmd.arg("lint")
        .assert()
        .failure()
        .stderr(predicates::str::contains("cannot read config file"));
}

#[test]
fn cvk_json_takes_precedence_over_cvk_jsonc_with_warning() {
    // multiple-configs/cvk.json disables no-undefined-variable-use while cvk.jsonc is empty,
    // so lint passes only if cvk.json is used.
    let root = std::path::Path::new(FIXTURES).join("multiple-configs");
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(&root);
    cmd.arg("lint")
        .assert()
        .success()
        .stderr(predicates::str::contains(format!(
            "warning: multiple config files found; using {} and ignoring {}",
            root.join("cvk.json").display(),
            root.join("cvk.jsonc").display(),
        )));
}
