use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "cvk",
    about = "A toolkit for CSS variables",
    version,
    help_template = CLI_HELP_TEMPLATE,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Lint CSS files for CSS variable issues
    Lint(LintArgs),
    /// Start the Language Server Protocol server
    Lsp,
}

#[derive(clap::Args, Default)]
#[command(
    about = "Lint CSS files for CSS variable issues",
    long_about = LINT_LONG_ABOUT,
    help_template = LINT_HELP_TEMPLATE,
)]
pub struct LintArgs {
    /// Override the root directory for resolving file paths
    #[arg(long)]
    pub root_dir: Option<String>,

    /// Path to config file [default: cvk.json]
    #[arg(short = 'c', long)]
    pub config: Option<String>,

    /// Rule override (e.g. --rule no-undefined-variable-use=off)
    #[arg(long)]
    pub rule: Vec<String>,

    /// Files to lint (overrides lookupFiles in config)
    pub files: Vec<String>,
}

static CLI_HELP_TEMPLATE: &str = "\
{name} {version}
{about}

{usage-heading} {usage}

Commands:
{subcommands}

Options:
{options}

Use `cvk <COMMAND> --help` for more information about a command.";

static LINT_LONG_ABOUT: &str = "\
Lint CSS files for CSS variable issues.

Analyzes CSS files and reports problems such as undefined variable usage.
By default, files and rules are determined by the configuration file (cvk.json).
You can override them with command-line options.";

static LINT_HELP_TEMPLATE: &str = "\
{about-with-newline}
{usage-heading} {usage}

{all-args}";
