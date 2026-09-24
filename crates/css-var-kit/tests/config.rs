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
    cmd.arg("lint").assert().success();
}

#[test]
fn config_with_trailing_commas_is_parsed_when_specified() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/trailing-comma"));
    cmd.args(["lint", "-c", "cvk.jsonc"]).assert().success();
}

#[test]
fn config_with_utf8_bom_is_parsed() {
    // bom/cvk.json starts with a UTF-8 BOM and disables no-undefined-variable-use,
    // so style.css (which uses an undefined variable) should pass.
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/bom"));
    cmd.arg("lint").assert().success();
}
