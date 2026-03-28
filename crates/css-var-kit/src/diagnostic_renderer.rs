use std::fmt::Write;

use yansi::Paint;

use crate::rules::{Diagnostic, Severity};

const HEADER_WIDTH: usize = 80;

pub fn render(diagnostic: &Diagnostic<'_>) -> String {
    let mut out = String::new();
    let char_column =
        byte_column_to_char_column(diagnostic.source, diagnostic.line, diagnostic.column);
    let lines: Vec<&str> = diagnostic.source.lines().collect();

    render_header(&mut out, diagnostic, char_column);
    render_message(&mut out, diagnostic);
    render_snippet(&mut out, diagnostic, &lines, char_column);

    out
}

fn render_header(out: &mut String, d: &Diagnostic<'_>, char_column: u32) {
    let location = format!(
        "{}:{}:{}",
        d.file_path.display(),
        d.line + 1,
        char_column + 1,
    );
    let prefix = format!("{location} {}", d.rule_name);
    let fill_len = HEADER_WIDTH.saturating_sub(prefix.len() + 1);
    let fill: String = "━".repeat(fill_len);
    let line = format!("{location} {} {fill}", d.rule_name);
    let _ = writeln!(out, "{}", severity_paint(d.severity, &line).bold());
}

fn render_message(out: &mut String, d: &Diagnostic<'_>) {
    let icon = match d.severity {
        Severity::Error => severity_paint(d.severity, "✖"),
        Severity::Warning => severity_paint(d.severity, "⚠"),
    };
    let _ = writeln!(out);
    let _ = writeln!(out, "  {icon} {}", Paint::new(&d.message).bold());
    let _ = writeln!(out);
}

fn render_snippet(out: &mut String, d: &Diagnostic<'_>, lines: &[&str], char_column: u32) {
    let target = d.line as usize;
    let start = target.saturating_sub(2);
    let end = (target + 2).min(lines.len());
    let max_line_num = end;
    let gutter_width = digit_count(max_line_num as u32);

    for (i, &content) in lines.iter().enumerate().take(end).skip(start) {
        let line_num = i + 1;
        let gutter_sep = "│".dim();

        if i == target {
            let marker = severity_paint(d.severity, ">");
            let _ = writeln!(
                out,
                "  {marker} {line_num:>gutter_width$} {gutter_sep} {content}",
                gutter_width = gutter_width,
            );
            let char_span = d
                .span_length
                .map(|byte_len| byte_span_to_char_len(content, d.column, byte_len));
            let underline = render_underline(content, char_column, char_span, d.severity);
            let _ = writeln!(
                out,
                "    {blank:>gutter_width$} {gutter_sep} {underline}",
                blank = "",
                gutter_width = gutter_width,
            );
        } else {
            let _ = writeln!(
                out,
                "    {line_num:>gutter_width$} {gutter_sep} {content}",
                gutter_width = gutter_width,
            );
        }
    }

    let _ = writeln!(out);
}

fn render_underline(
    line_content: &str,
    char_column: u32,
    char_span: Option<u32>,
    severity: Severity,
) -> String {
    let chars: Vec<char> = line_content.chars().collect();
    let col = char_column as usize;
    let span_len = match char_span {
        Some(len) => (len as usize).min(chars.len().saturating_sub(col)),
        None => chars.len().saturating_sub(col),
    };
    if span_len == 0 {
        return String::new();
    }
    let padding: String = " ".repeat(col);
    let carets: String = "^".repeat(span_len);
    format!("{padding}{}", severity_paint(severity, &carets))
}

fn byte_span_to_char_len(line_content: &str, byte_column: u32, byte_len: u32) -> u32 {
    let bytes = line_content.as_bytes();
    let start = (byte_column as usize).min(bytes.len());
    let end = (start + byte_len as usize).min(bytes.len());
    line_content[start..end].chars().count() as u32
}

fn severity_paint(severity: Severity, value: &str) -> yansi::Painted<&str> {
    match severity {
        Severity::Error => value.red(),
        Severity::Warning => value.yellow(),
    }
}

fn digit_count(n: u32) -> usize {
    if n == 0 {
        return 1;
    }
    ((n as f64).log10().floor() as usize) + 1
}

fn byte_column_to_char_column(source: &str, line: u32, byte_column: u32) -> u32 {
    let mut line_start = 0;
    let bytes = source.as_bytes();
    let mut current_line = 0u32;
    while current_line < line && line_start < bytes.len() {
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
    let byte_end = line_start + byte_column as usize;
    source[line_start..byte_end].chars().count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn make_diagnostic<'a>(
        source: &'a str,
        line: u32,
        column: u32,
        severity: Severity,
    ) -> Diagnostic<'a> {
        Diagnostic {
            file_path: Path::new("test.css"),
            source,
            line,
            span_length: None,
            column,
            rule_name: "test-rule",
            message: "test message".into(),
            severity,
        }
    }

    #[test]
    fn header_contains_file_and_rule() {
        let d = make_diagnostic(".a { color: red; }", 0, 12, Severity::Error);
        let output = render(&d);
        assert!(output.contains("test.css:1:13"));
        assert!(output.contains("test-rule"));
        assert!(output.contains("━"));
    }

    #[test]
    fn error_uses_cross_icon() {
        let d = make_diagnostic(".a { color: red; }", 0, 12, Severity::Error);
        let output = render(&d);
        assert!(output.contains("✖"));
    }

    #[test]
    fn warning_uses_warning_icon() {
        let d = make_diagnostic(".a { color: red; }", 0, 12, Severity::Warning);
        let output = render(&d);
        assert!(output.contains("⚠"));
    }

    #[test]
    fn snippet_shows_target_line_with_marker() {
        let source = ".a {\n  color: red;\n}\n";
        let d = make_diagnostic(source, 1, 2, Severity::Error);
        let output = render(&d);
        assert!(output.contains(">"));
        assert!(output.contains("color: red;"));
    }

    #[test]
    fn underline_has_carets() {
        let source = ".a {\n  color: red;\n}\n";
        let d = make_diagnostic(source, 1, 2, Severity::Error);
        let output = render(&d);
        assert!(output.contains("^"));
    }

    #[test]
    fn byte_column_to_char_ascii() {
        assert_eq!(byte_column_to_char_column(".a { color: red; }", 0, 5), 5);
    }

    #[test]
    fn byte_column_to_char_multibyte() {
        assert_eq!(byte_column_to_char_column(".あ { --color: red; }", 0, 7), 5);
    }
}
