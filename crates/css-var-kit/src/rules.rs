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
