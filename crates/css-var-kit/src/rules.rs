use std::path::Path;

pub mod undefined_variables;

pub struct Diagnostic<'a> {
    pub file_path: &'a Path,
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub severity: Severity,
}

pub enum Severity {
    Error,
    Warning,
}
