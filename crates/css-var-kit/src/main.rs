use std::env;
use std::process;

use clap::Parser;

use css_var_kit::cli::{Cli, Command};
use css_var_kit::commands;
use css_var_kit::config::Config;

fn main() {
    yansi::whenever(yansi::Condition::STDERR_IS_TTY);
    let cli = Cli::parse();
    let cwd = env::current_dir().expect("failed to get current directory");

    match cli.command {
        Command::Lint(args) => {
            let config = Config::load(&cwd, Some(args)).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                process::exit(1);
            });
            commands::lint::run(&config);
        }
        Command::Lsp => {
            commands::lsp::run(&cwd).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                process::exit(1);
            });
        }
    }
}
