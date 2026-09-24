use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;
use serde::de::{self, Deserializer};

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
    pub fn load(project_root: &Path) -> Result<Option<Self>, ConfigError> {
        CONFIG_FILENAMES
            .iter()
            .map(|name| project_root.join(name))
            .find_map(|path| match fs::read_to_string(&path) {
                Ok(raw) => Some(Self::parse(&path, raw)),
                Err(e) if e.kind() == ErrorKind::NotFound => None,
                Err(e) => Some(Err(ConfigError::ReadFile { path, source: e })),
            })
            .transpose()
    }

    pub(super) fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path).map_err(|e| ConfigError::ReadFile {
            path: path.to_path_buf(),
            source: e,
        })?;
        Self::parse(path, raw)
    }

    /// Strips the whole content at once because the streaming `StripComments` reader
    /// cannot detect trailing commas when `serde_json::from_reader` reads byte by byte.
    fn parse(path: &Path, mut raw: String) -> Result<Self, ConfigError> {
        let bom_len = if raw.starts_with(UTF8_BOM) {
            UTF8_BOM.len_utf8()
        } else {
            0
        };
        let json = &mut raw[bom_len..];
        json_strip_comments::strip(json)
            .map_err(serde_json::Error::io)
            .and_then(|()| serde_json::from_str(json))
            .map_err(|e| ConfigError::Parse {
                path: path.to_path_buf(),
                source: e,
            })
    }
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

fn default_root_dir() -> String {
    ".".to_string()
}

fn default_lookup_files() -> Vec<String> {
    vec!["**/*.css".to_string()]
}

fn default_error() -> SeverityToggle {
    SeverityToggle::Error
}
