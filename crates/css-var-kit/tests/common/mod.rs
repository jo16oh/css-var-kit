#![allow(dead_code)]

pub mod lsp_client;

use assert_cmd::Command;

pub const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

pub fn cvk() -> Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/default"));
    cmd
}
