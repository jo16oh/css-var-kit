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
