mod cli;
mod commands;
mod config;
mod diagnostic_renderer;
mod owned;
mod parser;
mod position;
mod rules;
mod searcher;
mod type_checker;
mod variable_resolver;

use std::env;
use std::process;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::config::Config;

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
        Command::Lsp(args) => {
            commands::lsp::run(&cwd, args.log).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                process::exit(1);
            });
        }
    }
}
