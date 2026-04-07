use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use lsp_types::notification::PublishDiagnostics;
use lsp_types::{DiagnosticSeverity, NumberOrString, Position, PublishDiagnosticsParams, Range};

use super::Server;
use crate::commands::lint;
use crate::commands::lsp::uri::path_to_uri;
use crate::parser;
use crate::position::byte_col_to_utf16_in_source;
use crate::rules::{Diagnostic, Severity};

impl Server<'_> {
    pub fn publish_diagnostics(&self) -> Result<(), Box<dyn Error>> {
        dbg!(&self.source_cache);
        let sources: Vec<(&Path, &str)> = self
            .source_cache
            .iter()
            .map(|(path, content)| (path.as_path(), content.as_str()))
            .collect();

        let parse_results: Vec<_> = sources
            .iter()
            .map(|(path, content)| parser::css::parse(content, path))
            .collect();

        let diagnostics = lint::check(&parse_results, self.config);

        self.log(&format!(
            "publishDiagnostics: {} files, {} diagnostics total",
            sources.len(),
            diagnostics.len()
        ));

        let mut by_file: HashMap<&Path, Vec<lsp_types::Diagnostic>> = HashMap::new();
        for d in &diagnostics {
            by_file
                .entry(d.file_path)
                .or_default()
                .push(to_lsp_diagnostic(d));
        }

        for (path, _) in &sources {
            let lsp_diagnostics = by_file.remove(*path).unwrap_or_default();
            let abs_path = self.config.root_dir.join(path);
            let uri = path_to_uri(&abs_path);
            self.send_notification::<PublishDiagnostics>(PublishDiagnosticsParams {
                uri,
                diagnostics: lsp_diagnostics,
                version: None,
            })?;
        }

        Ok(())
    }
}

fn to_lsp_diagnostic(d: &Diagnostic<'_>) -> lsp_types::Diagnostic {
    let start = Position {
        line: d.line,
        character: byte_col_to_utf16_in_source(d.source, d.line, d.column),
    };

    let end = match d.span_length {
        Some(len) => Position {
            line: d.line,
            character: byte_col_to_utf16_in_source(d.source, d.line, d.column + len),
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
                character: byte_col_to_utf16_in_source(d.source, d.line, line_end_col),
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
