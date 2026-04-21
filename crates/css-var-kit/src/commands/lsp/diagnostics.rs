mod pull;
mod push;

use lsp_types::{DiagnosticSeverity, NumberOrString, Position, Range};

use super::Server;
use crate::rules::{Diagnostic, Severity};
use crate::text_position::byte_col_to_utf16_in_source;

pub fn to_lsp_diagnostic(d: &Diagnostic) -> lsp_types::Diagnostic {
    let start = Position {
        line: d.line,
        character: byte_col_to_utf16_in_source(&d.source, d.line, d.column),
    };

    let end = match d.span_length {
        Some(len) => Position {
            line: d.line,
            character: byte_col_to_utf16_in_source(&d.source, d.line, d.column + len),
        },
        None => {
            let line_end_col = d
                .source
                .lines()
                .nth(d.line as usize)
                .map(|line| line.len() as u32)
                .unwrap_or(d.column + 1);
            Position {
                line: d.line,
                character: byte_col_to_utf16_in_source(&d.source, d.line, line_end_col),
            }
        }
    };

    lsp_types::Diagnostic {
        range: Range { start, end },
        severity: Some(match d.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
        }),
        code: Some(NumberOrString::String(d.rule_name.to_owned())),
        source: Some("cvk".to_owned()),
        message: d.message.clone(),
        ..Default::default()
    }
}
