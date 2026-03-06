use std::env;
use std::path::{Path, PathBuf};
use std::process;

use css_var_kit::commands;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cwd = env::current_dir().expect("failed to get current directory");
    let project_root = find_project_root(&cwd);

    match args.get(1).map(|s| s.as_str()) {
        Some("lint") => {
            commands::lint::run(&project_root, &args[2..]);
        }
        _ => {
            print_help();
            process::exit(1);
        }
    }
}

fn find_project_root(cwd: &Path) -> PathBuf {
    let markers = ["cvk.json", "package.json", ".git"];

    for marker in markers {
        if let Some(root) = find_ancestor_with(cwd, marker) {
            return root;
        }
    }

    cwd.to_path_buf()
}

fn find_ancestor_with(start: &Path, name: &str) -> Option<PathBuf> {
    let mut dir = start;
    loop {
        if dir.join(name).exists() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

fn print_help() {
    eprint!("{}", include_str!("help.txt"));
}
