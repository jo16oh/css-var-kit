use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde::de::{self, Deserializer};

use super::ConfigError;
use crate::rules::Severity;
use crate::rules::enforce_variable_use::config::RawEnforceVariableUse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawConfig {
    #[serde(default = "default_root_dir")]
    pub(super) root_dir: String,
    #[serde(default = "default_lookup_files")]
    pub(super) lookup_files: Vec<String>,
    #[serde(default)]
    pub(super) rules: RawRules,
    #[serde(default)]
    pub(crate) lsp: RawLspConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawLspConfig {
    pub(crate) log_file: Option<String>,
}

impl Default for RawConfig {
    fn default() -> Self {
        Self {
            root_dir: default_root_dir(),
            lookup_files: default_lookup_files(),
            rules: RawRules::default(),
            lsp: RawLspConfig::default(),
        }
    }
}

impl RawConfig {
    pub(super) fn load(project_root: &Path) -> Result<Self, ConfigError> {
        let candidates = ["cvk.json", "cvk.jsonc"];

        for name in candidates {
            let path = project_root.join(name);
            if let Ok(raw) = fs::read_to_string(&path) {
                let stripped = json_strip_comments::StripComments::new(raw.as_bytes());
                return serde_json::from_reader(stripped)
                    .map_err(|e| ConfigError::Parse { path, source: e });
            }
        }

        Ok(Self::default())
    }

    pub(super) fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path).map_err(|e| ConfigError::ReadFile {
            path: path.to_path_buf(),
            source: e,
        })?;
        let stripped = json_strip_comments::StripComments::new(raw.as_bytes());
        serde_json::from_reader(stripped).map_err(|e| ConfigError::Parse {
            path: path.to_path_buf(),
            source: e,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct RawRules {
    #[serde(default = "default_error")]
    pub(super) no_undefined_variable_use: SeverityToggle,
    #[serde(default = "default_error")]
    pub(super) no_variable_type_mismatch: SeverityToggle,
    #[serde(default = "default_error")]
    pub(super) no_inconsistent_variable_definition: SeverityToggle,
    #[serde(default)]
    pub(super) enforce_variable_use: RawEnforceVariableUse,
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
