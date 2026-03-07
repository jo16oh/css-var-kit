use std::path::Path;

pub mod undefined_variables;

pub struct Diagnostic<'src> {
    pub file_path: &'src Path,
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub severity: Severity,
}

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
        eprintln!(
            "{}:{}:{}: {}: {}",
            self.file_path.display(),
            self.line + 1,
            self.column + 1,
            severity,
            self.message,
        );
    }
}
