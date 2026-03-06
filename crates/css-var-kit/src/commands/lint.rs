use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use crate::parser;
use crate::rules::undefined_variables;
use crate::rules::{Diagnostic, Severity};
use crate::searcher::SearcherBuilder;
use crate::searcher::conditions::variable_definitions::{
    VariableDefinitionMap, VariableDefinitions,
};
use crate::searcher::conditions::variable_usages::VariableUsages;

const SKIP_DIRS: &[&str] = &["node_modules", "target", ".git", "dist", "build", "vendor"];

pub fn run(dir: &Path, _args: &[String]) {
    let css_files = collect_css_files(dir);

    if css_files.is_empty() {
        return;
    }

    let sources: Vec<(PathBuf, String)> = css_files
        .into_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok()?;
            Some((path, content))
        })
        .collect();

    let parse_results: Vec<_> = sources
        .iter()
        .map(|(path, content)| parser::css::parse(content.as_str(), path.as_path()))
        .collect();

    let searcher = SearcherBuilder::new(&parse_results)
        .add_condition(VariableDefinitions)
        .add_condition(VariableUsages)
        .build();
    let search_result = searcher.search();

    let defs = search_result.get_result_for(VariableDefinitions);
    let def_map = VariableDefinitionMap::from(&defs);

    let usages = search_result.get_result_for(VariableUsages);
    let diagnostics = undefined_variables::check(&def_map, &usages);

    if diagnostics.is_empty() {
        return;
    }

    for d in &diagnostics {
        print_diagnostic(d);
    }

    process::exit(1);
}

fn print_diagnostic(d: &Diagnostic) {
    let severity = match d.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    eprintln!(
        "{}:{}:{}: {}: {}",
        d.file_path.display(),
        d.line + 1,
        d.column + 1,
        severity,
        d.message,
    );
}

fn collect_css_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_css_files_recursive(dir, dir, &mut files);
    files.sort();
    files
}

fn collect_css_files_recursive(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) {
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
            collect_css_files_recursive(root, &path, files);
        } else if path.extension().is_some_and(|ext| ext == "css") {
            files.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        }
    }
}
