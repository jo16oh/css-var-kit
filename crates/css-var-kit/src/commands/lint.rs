use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use crate::config::Config;
use crate::parser;
use crate::rules::{Diagnostic, Severity};
use crate::searcher::SearcherBuilder;

const SKIP_DIRS: &[&str] = &["node_modules", "target", ".git", "dist", "build", "vendor"];
const HTML_LIKE_EXTENSIONS: &[&str] = &["vue", "svelte", "astro"];

pub fn run(config: &Config) {
    let css_files = collect_source_files(config.root_dir.as_path());

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
        .flat_map(|(path, content)| parse_file(content.as_str(), path.as_path()))
        .collect();

    let diagnostics = check(&parse_results, config);

    if diagnostics.is_empty() {
        return;
    }

    for d in &diagnostics {
        d.print();
    }

    if diagnostics
        .iter()
        .any(|d| matches!(d.severity, Severity::Error))
    {
        process::exit(1);
    }
}

pub fn check<'src>(
    parse_results: &'src [parser::css::ParseResult<'src>],
    config: &Config,
) -> Vec<Diagnostic<'src>> {
    let compiled_rules = config.rules.compile(config);

    let mut searcher = SearcherBuilder::new(parse_results);
    for rule in &compiled_rules {
        searcher = rule.register_conditions(searcher);
    }

    let search_result = searcher.build().search();

    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for rule in &compiled_rules {
        diagnostics.extend(rule.check(&search_result));
    }

    diagnostics
}

pub fn collect_source_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_source_files_recursive(dir, &mut files);
    files.sort();
    files
}

fn collect_source_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
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
            collect_source_files_recursive(&path, files);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext == "css" || HTML_LIKE_EXTENSIONS.contains(&ext))
        {
            files.push(path);
        }
    }
}

pub fn parse_file<'src>(
    content: &'src str,
    path: &'src Path,
) -> Vec<parser::css::ParseResult<'src>> {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) if HTML_LIKE_EXTENSIONS.contains(&ext) => {
            parser::html_like::parse_html_like(content, path)
        }
        _ => vec![parser::css::parse(content, path)],
    }
}
