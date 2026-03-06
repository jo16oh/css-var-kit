use std::env;
use std::process;

use css_var_kit::commands;

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("lint") => {
            let dir = env::current_dir().expect("failed to get current directory");
            commands::lint::run(&dir, &args[2..]);
        }
        _ => {
            print_help();
            process::exit(1);
        }
    }
}

fn print_help() {
    eprintln!("Usage: cvk <command>");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  lint    Lint CSS files for undefined variables");
}
