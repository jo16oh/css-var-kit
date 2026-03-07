use std::env;
use std::process;

use css_var_kit::commands;
use css_var_kit::config::Config;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cwd = env::current_dir().expect("failed to get current directory");
    let cfg = Config::load(&cwd).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        process::exit(1);
    });

    match args.get(1).map(|s| s.as_str()) {
        Some("lint") => {
            commands::lint::run(&cfg.root_dir, &args[2..]);
        }
        _ => {
            print_help();
            process::exit(1);
        }
    }
}

fn print_help() {
    eprint!("{}", include_str!("help.txt"));
}
