use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cvk", about = "A toolkit for CSS custom properties")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Lint CSS files
    Lint(LintArgs),
}

#[derive(clap::Args)]
pub struct LintArgs {
    /// Override the root directory
    #[arg(long)]
    pub root_dir: Option<String>,

    /// Path to config file
    #[arg(short = 'c', long)]
    pub config: Option<String>,

    /// Rule overrides (e.g. --rule no-undefined-variable-use=off)
    #[arg(long)]
    pub rule: Vec<String>,

    /// Files to lint (overrides lookupFiles)
    pub files: Vec<String>,
}
