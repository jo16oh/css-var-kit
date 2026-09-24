use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::{self, DeserializeOwned, Deserializer};

use super::ConfigError;
use crate::file_kinds::CONFIG_FILENAMES;
use crate::rules::Severity;
use crate::rules::enforce_variable_use::config::RawEnforceVariableUse;

const UTF8_BOM: char = '\u{feff}';

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawConfig {
    #[serde(default = "default_root_dir")]
    pub root_dir: String,
    #[serde(default = "default_lookup_files")]
    pub lookup_files: Vec<String>,
    // If present, overrides lookup_files.
    #[serde(default)]
    pub definition_files: Option<Vec<String>>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub rules: RawRules,
    #[serde(default)]
    pub lsp: RawLspConfig,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawLspConfig {
    pub log_file: Option<String>,
}

impl Default for RawConfig {
    fn default() -> Self {
        Self {
            root_dir: default_root_dir(),
            lookup_files: default_lookup_files(),
            definition_files: None,
            include: vec![],
            rules: RawRules::default(),
            lsp: RawLspConfig::default(),
        }
    }
}

impl RawConfig {
    /// Searches for `cvk.json` or `cvk.jsonc` in `project_root`.
    /// Returns `Ok(Some(config))` if found, `Ok(None)` if no config file exists.
    /// If both exist, `cvk.json` is used and a warning is printed.
    pub fn load(project_root: &Path) -> Result<Option<Self>, ConfigError> {
        let existing: Vec<PathBuf> = CONFIG_FILENAMES
            .iter()
            .map(|name| project_root.join(name))
            .filter(|path| path.is_file())
            .collect();

        if let [used, ignored @ ..] = existing.as_slice()
            && !ignored.is_empty()
        {
            warn_multiple_config_files(used, ignored);
        }

        existing
            .first()
            .map(|path| Self::load_from(path))
            .transpose()
    }

    pub(super) fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path).map_err(|e| ConfigError::ReadFile {
            path: path.to_path_buf(),
            source: e,
        })?;
        parse_jsonc(raw).map_err(|e| ConfigError::Parse {
            path: path.to_path_buf(),
            source: e,
        })
    }
}

/// Parses JSON that may contain a leading BOM, comments and trailing commas.
///
/// Strips the whole content at once because the streaming `StripComments` reader
/// cannot detect trailing commas when `serde_json::from_reader` reads byte by byte.
/// The in-place `strip` does not report a comment left open at EOF, so the streaming
/// reader is still run first to reject it.
pub(super) fn parse_jsonc<T: DeserializeOwned>(mut raw: String) -> serde_json::Result<T> {
    let bom_len = if raw.starts_with(UTF8_BOM) {
        UTF8_BOM.len_utf8()
    } else {
        0
    };
    let json = &mut raw[bom_len..];
    io::copy(
        &mut json_strip_comments::StripComments::new(json.as_bytes()),
        &mut io::sink(),
    )
    .and_then(|_| json_strip_comments::strip(json))
    .map_err(serde_json::Error::io)
    .and_then(|()| serde_json::from_str(json))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RawRules {
    #[serde(default = "default_error")]
    pub no_undefined_variable_use: SeverityToggle,
    #[serde(default = "default_error")]
    pub no_variable_type_mismatch: SeverityToggle,
    #[serde(default = "default_error")]
    pub no_inconsistent_variable_definition: SeverityToggle,
    #[serde(default)]
    pub enforce_variable_use: RawEnforceVariableUse,
}

impl Default for RawRules {
    fn default() -> Self {
        Self {
            no_undefined_variable_use: SeverityToggle::Error,
            enforce_variable_use: RawEnforceVariableUse::Off,
            no_variable_type_mismatch: SeverityToggle::Error,
            no_inconsistent_variable_definition: SeverityToggle::Error,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SeverityToggle {
    Error,
    Warn,
    Off,
}

impl SeverityToggle {
    pub fn severity(self) -> Option<Severity> {
        match self {
            Self::Error => Some(Severity::Error),
            Self::Warn => Some(Severity::Warning),
            Self::Off => None,
        }
    }
}

impl<'de> Deserialize<'de> for SeverityToggle {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "on" | "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            "off" => Ok(Self::Off),
            _ => Err(de::Error::unknown_variant(
                &s,
                &["error", "warn", "on", "off"],
            )),
        }
    }
}

fn warn_multiple_config_files(used: &Path, ignored: &[PathBuf]) {
    let ignored_names = ignored
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    eprintln!(
        "warning: multiple config files found; using {} and ignoring {ignored_names}",
        used.display()
    );
}

fn default_root_dir() -> String {
    ".".to_string()
}

fn default_lookup_files() -> Vec<String> {
    vec!["**/*.css".to_string()]
}

fn default_error() -> SeverityToggle {
    SeverityToggle::Error
}
