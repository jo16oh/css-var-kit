use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use crate::config::Config;
use crate::parser;
use crate::rules::Diagnostic;
use crate::rules::Rule;
use crate::rules::undefined_variables::NoUndefinedVariableUse;
use crate::searcher::SearcherBuilder;

const SKIP_DIRS: &[&str] = &["node_modules", "target", ".git", "dist", "build", "vendor"];

pub fn run(config: &Config, _args: &[String]) {
    let css_files = collect_css_files(config.root_dir.as_path());

    if css_files.is_empty() {
        return;
    }

    let sources: Vec<(PathBuf, String)> = css_files
        .into_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok()?;
            let rel_path = path
                .strip_prefix(config.root_dir.as_path())
                .unwrap_or(&path)
                .to_path_buf();
            Some((rel_path, content))
        })
        .collect();

    let parse_results: Vec<_> = sources
        .iter()
        .map(|(path, content)| parser::css::parse(content.as_str(), path.as_path()))
        .collect();

    let mut rules: Vec<Box<dyn Rule>> = Vec::new();

    if config.rules.no_undefined_variable_use {
        rules.push(Box::new(NoUndefinedVariableUse));
    }

    let mut searcher = SearcherBuilder::new(&parse_results);
    for rule in &rules {
        searcher = rule.register_conditions(searcher);
    }

    let search_result = searcher.build().search();

    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for rule in &rules {
        diagnostics.extend(rule.check(&search_result));
    }

    if diagnostics.is_empty() {
        return;
    }

    for d in &diagnostics {
        d.print();
    }

    process::exit(1);
}

fn collect_css_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_css_files_recursive(dir, &mut files);
    files.sort();
    files
}

fn collect_css_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if SKIP_DIRS.contains(&name) {
                    continue;
                }
            }
            collect_css_files_recursive(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "css") {
            files.push(path);
        }
    }
}
