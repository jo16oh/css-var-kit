use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::ConfigError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawConfig {
    #[serde(default = "default_root_dir")]
    pub(super) root_dir: String,
    #[serde(default = "default_lookup_files")]
    pub(super) lookup_files: Vec<String>,
    #[serde(default)]
    pub(super) rules: RawRules,
}

impl Default for RawConfig {
    fn default() -> Self {
        Self {
            root_dir: default_root_dir(),
            lookup_files: default_lookup_files(),
            rules: RawRules::default(),
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

    pub(in crate::config) fn load_from(path: &Path) -> Result<Self, ConfigError> {
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
pub(in crate::config) struct RawRules {
    #[serde(default = "default_on")]
    pub(in crate::config) no_undefined_variable_use: Toggle,
    #[serde(default = "default_on")]
    pub(in crate::config) no_compound_value_in_definition: Toggle,
    #[serde(default = "default_on")]
    pub(in crate::config) no_variable_type_mismatch: Toggle,
    #[serde(default = "default_on")]
    pub(in crate::config) no_inconsistent_variable_definition: Toggle,
    #[serde(default)]
    pub(in crate::config) enforce_variable_use: RawEnforceVariableUse,
}

impl Default for RawRules {
    fn default() -> Self {
        Self {
            no_undefined_variable_use: Toggle::On,
            enforce_variable_use: RawEnforceVariableUse::Off,
            no_compound_value_in_definition: Toggle::On,
            no_variable_type_mismatch: Toggle::On,
            no_inconsistent_variable_definition: Toggle::On,
        }
    }
}

#[derive(Default, Debug, Deserialize)]
#[serde(untagged)]
pub(in crate::config) enum RawEnforceVariableUse {
    #[serde(rename = "off")]
    #[default]
    Off,
    Config(RawEnforceVariableUseConfig),
}

impl RawEnforceVariableUse {
    pub(in crate::config) fn default_on() -> Self {
        Self::Config(RawEnforceVariableUseConfig::default())
    }

    pub(in crate::config) fn into_config(self) -> Option<RawEnforceVariableUseConfig> {
        match self {
            Self::Off => None,
            Self::Config(config) => Some(config),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::config) struct RawEnforceVariableUseConfig {
    #[serde(default)]
    pub(in crate::config) types: Vec<String>,
    #[serde(default = "default_allowed_functions")]
    pub(in crate::config) allowed_functions: Vec<String>,
    #[serde(default = "default_allowed_values")]
    pub(in crate::config) allowed_values: Vec<String>,
}

impl Default for RawEnforceVariableUseConfig {
    fn default() -> Self {
        Self {
            types: Vec::new(),
            allowed_functions: default_allowed_functions(),
            allowed_values: default_allowed_values(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(in crate::config) enum Toggle {
    On,
    Off,
}

impl Toggle {
    pub(in crate::config) fn is_on(&self) -> bool {
        matches!(self, Toggle::On)
    }
}

fn default_root_dir() -> String {
    ".".to_string()
}

fn default_lookup_files() -> Vec<String> {
    vec!["**/*.css".to_string()]
}

fn default_on() -> Toggle {
    Toggle::On
}

fn default_allowed_functions() -> Vec<String> {
    ["calc", "min", "max", "clamp", "env"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_allowed_values() -> Vec<String> {
    [
        "inherit",
        "initial",
        "unset",
        "revert",
        "revert-layer",
        "currentColor",
        "transparent",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
