use std::path::Path;

use crate::searcher::{SearchResult, SearcherBuilder};

pub mod enforce_variable_use;
pub mod no_inconsistent_variable_definition;
pub mod no_undefined_variable_use;
pub mod no_variable_type_mismatch;

pub trait Rule {
    fn register_conditions<'src>(&self, searcher: SearcherBuilder<'src>) -> SearcherBuilder<'src>;

    fn check<'src>(&self, search_result: &SearchResult<'src>) -> Vec<Diagnostic<'src>>;
}

pub fn is_ignored(ignore_comments: &[&str], rule_name: &str) -> bool {
    ignore_comments.iter().any(|&comment| {
        if comment == "cvk-ignore" {
            return true;
        }
        if let Some(rest) = comment.strip_prefix("cvk-ignore:") {
            return rest.trim() == rule_name;
        }
        false
    })
}

pub struct Diagnostic<'src> {
    pub file_path: &'src Path,
    pub source: &'src str,
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Error,
    Warning,
}

impl<'src> Diagnostic<'src> {
    pub fn print(&self) {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let char_column = self.byte_column_to_char_column();
        eprintln!(
            "{}:{}:{}: {}: {}",
            self.file_path.display(),
            self.line + 1,
            char_column + 1,
            severity,
            self.message,
        );
    }

    fn byte_column_to_char_column(&self) -> u32 {
        let mut line_start = 0;
        let bytes = self.source.as_bytes();
        let mut current_line = 0u32;
        while current_line < self.line && line_start < bytes.len() {
            match bytes[line_start] {
                b'\r' => {
                    line_start += 1;
                    if line_start < bytes.len() && bytes[line_start] == b'\n' {
                        line_start += 1;
                    }
                    current_line += 1;
                }
                b'\n' => {
                    line_start += 1;
                    current_line += 1;
                }
                _ => {
                    line_start += 1;
                }
            }
        }
        let byte_end = line_start + self.column as usize;
        self.source[line_start..byte_end].chars().count() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diagnostic<'a>(source: &'a str, line: u32, column: u32) -> Diagnostic<'a> {
        Diagnostic {
            file_path: Path::new("test.css"),
            source,
            line,
            column,
            message: String::new(),
            severity: Severity::Warning,
        }
    }

    #[test]
    fn ascii_column_unchanged() {
        let d = make_diagnostic(".a { color: red; }", 0, 5);
        assert_eq!(d.byte_column_to_char_column(), 5);
    }

    #[test]
    fn multibyte_column_converted() {
        // ".あ { " = 7 bytes, but 5 chars
        let d = make_diagnostic(".あ { --color: red; }", 0, 7);
        assert_eq!(d.byte_column_to_char_column(), 5);
    }

    #[test]
    fn multibyte_on_second_line() {
        let source = ".あ { }\n.b { --color: red; }";
        // line 1, column 5 (byte offset of "--color" on that line)
        let d = make_diagnostic(source, 1, 5);
        assert_eq!(d.byte_column_to_char_column(), 5);
    }

    #[test]
    fn crlf_line_endings() {
        let source = ".a { }\r\n.b { --color: red; }";
        let d = make_diagnostic(source, 1, 5);
        assert_eq!(d.byte_column_to_char_column(), 5);
    }

    #[test]
    fn emoji_4byte_char() {
        // "🎨" is 4 bytes but 1 char
        let d = make_diagnostic(".🎨 { --c: red; }", 0, 7);
        // ".🎨 { " = 1 + 4 + 1 + 1 = 7 bytes, 1 + 1 + 1 + 1 = 4 chars
        assert_eq!(d.byte_column_to_char_column(), 4);
    }

    #[test]
    fn is_ignored_bare_cvk_ignore() {
        assert!(is_ignored(&["cvk-ignore"], "any-rule"));
    }

    #[test]
    fn is_ignored_matching_rule_name() {
        assert!(is_ignored(&["cvk-ignore: my-rule"], "my-rule"));
    }

    #[test]
    fn is_ignored_non_matching_rule_name() {
        assert!(!is_ignored(&["cvk-ignore: other-rule"], "my-rule"));
    }

    #[test]
    fn is_ignored_empty() {
        assert!(!is_ignored(&[], "my-rule"));
    }

    #[test]
    fn is_ignored_multiple_comments() {
        assert!(is_ignored(
            &["cvk-ignore: rule-a", "cvk-ignore: rule-b"],
            "rule-b"
        ));
    }
}
