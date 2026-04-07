#![allow(dead_code)]

pub mod lsp_client;

use std::{fs, path::Path};

use assert_cmd::Command;

pub const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

pub fn cvk() -> Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("cvk");
    cmd.current_dir(format!("{FIXTURES}/default"));
    cmd
}

pub fn copy_fixture_to_tempdir(fixture_name: &str) -> tempfile::TempDir {
    let fixture_dir = Path::new(FIXTURES).join(fixture_name);
    let tmp = tempfile::tempdir().unwrap();
    copy_dir_recursive(&fixture_dir, tmp.path());
    tmp
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) {
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let dest_path = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir_all(&dest_path).unwrap();
            copy_dir_recursive(&entry.path(), &dest_path);
        } else {
            fs::copy(entry.path(), dest_path).unwrap();
        }
    }
}
