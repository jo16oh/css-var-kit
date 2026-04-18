use std::{path::Path, rc::Rc};

use crate::{
    owned::OwnedStr,
    searcher::{SearchResult, SearcherBuilder},
};

pub mod enforce_variable_use;
pub mod no_inconsistent_variable_definition;
pub mod no_undefined_variable_use;
pub mod no_variable_type_mismatch;

pub trait Rule {
    fn register_conditions(&self, searcher: SearcherBuilder) -> SearcherBuilder;

    fn check(&self, search_result: &SearchResult) -> Vec<Diagnostic>;
}

pub fn is_ignored(ignore_comments: &[OwnedStr], rule_name: &str) -> bool {
    ignore_comments.iter().any(|comment| {
        if &**comment == "cvk-ignore" {
            return true;
        }
        if let Some(rest) = comment.strip_prefix("cvk-ignore:") {
            return rest.trim() == rule_name;
        }
        false
    })
}

pub struct Diagnostic {
    pub file_path: Rc<Path>,
    pub source: OwnedStr,
    pub line: u32,
    pub column: u32,
    pub span_length: Option<u32>,
    pub rule_name: &'static str,
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Error,
    Warning,
}

impl Diagnostic {
    pub fn print(&self) {
        eprint!("{}", crate::diagnostic_renderer::render(self));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_ignored_str(comments: &[&str], rule_name: &str) -> bool {
        let owned_comments: Vec<OwnedStr> = comments.iter().map(|c| OwnedStr::from(*c)).collect();
        is_ignored(&owned_comments, rule_name)
    }

    #[test]
    fn is_ignored_bare_cvk_ignore() {
        assert!(is_ignored_str(&["cvk-ignore"], "any-rule"));
    }

    #[test]
    fn is_ignored_matching_rule_name() {
        assert!(is_ignored_str(&["cvk-ignore: my-rule"], "my-rule"));
    }

    #[test]
    fn is_ignored_non_matching_rule_name() {
        assert!(!is_ignored_str(&["cvk-ignore: other-rule"], "my-rule"));
    }

    #[test]
    fn is_ignored_empty() {
        assert!(!is_ignored_str(&[], "my-rule"));
    }

    #[test]
    fn is_ignored_multiple_comments() {
        assert!(is_ignored_str(
            &["cvk-ignore: rule-a", ("cvk-ignore: rule-b")],
            "rule-b"
        ));
    }
}
