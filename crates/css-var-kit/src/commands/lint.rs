use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use crate::config::{Config, LookupFilesMatcher};
use crate::parser;
use crate::rules::{Diagnostic, Severity};
use crate::searcher::SearcherBuilder;

const HTML_LIKE_EXTENSIONS: &[&str] = &["html", "vue", "svelte", "astro"];

pub fn run(config: &Config) {
    let css_files = collect_source_files(config.root_dir.as_path(), &config.include);
    let include_files = collect_include_files(config.root_dir.as_path(), &config.include);

    if css_files.is_empty() && include_files.is_empty() {
        return;
    }

    let mut sources: Vec<(PathBuf, String)> = css_files
        .into_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok()?;
            let rel_path = path
                .strip_prefix(config.root_dir.as_path())
                .unwrap_or(&path)
                .to_path_buf();
            if config.include.is_negated(&rel_path) {
                return None;
            }
            Some((rel_path, content))
        })
        .collect();

    let include_sources: Vec<(PathBuf, String)> = include_files
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

    sources.extend(include_sources);

    let parse_results: Vec<_> = sources
        .iter()
        .flat_map(|(path, content)| parse_file(content.as_str(), path.as_path()))
        .collect();

    let diagnostics = check(&parse_results, config);
    let diagnostics: Vec<_> = diagnostics
        .into_iter()
        .filter(|d| !config.include.matches(d.file_path))
        .collect();

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

pub fn collect_source_files(dir: &Path, include: &LookupFilesMatcher) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_source_files_recursive(dir, dir, include, &mut files);
    files.sort();
    files
}

/// Collects files matching positive patterns in `include`, walking the full tree.
/// Returns an empty list when `include` has no positive patterns (the default).
pub fn collect_include_files(root: &Path, include: &LookupFilesMatcher) -> Vec<PathBuf> {
    if !include.has_positive_patterns() {
        return vec![];
    }
    let mut files = Vec::new();
    collect_include_recursive(root, root, include, &mut files);
    files.sort();
    files
}

fn collect_include_recursive(
    root: &Path,
    dir: &Path,
    include: &LookupFilesMatcher,
    files: &mut Vec<PathBuf>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_include_recursive(root, &path, include, files);
        } else if is_supported_extension(&path) {
            if let Ok(rel) = path.strip_prefix(root) {
                if include.matches(rel) {
                    files.push(path);
                }
            }
        }
    }
}

fn is_supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| matches!(ext, "css" | "scss") || HTML_LIKE_EXTENSIONS.contains(&ext))
}

fn collect_source_files_recursive(
    root: &Path,
    dir: &Path,
    include: &LookupFilesMatcher,
    files: &mut Vec<PathBuf>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            if include.is_dir_negated(rel) {
                continue;
            }
            collect_source_files_recursive(root, &path, include, files);
        } else if is_supported_extension(&path) {
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
