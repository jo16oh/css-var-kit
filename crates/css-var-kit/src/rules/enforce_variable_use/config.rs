use std::collections::HashSet;

use lightningcss::values::syntax::SyntaxComponentKind;
use serde::Deserialize;

use crate::config::ConfigError;

#[derive(Default, Debug, Deserialize)]
#[serde(untagged)]
pub enum RawEnforceVariableUse {
    #[serde(rename = "off")]
    #[default]
    Off,
    Config(RawEnforceVariableUseConfig),
}

impl RawEnforceVariableUse {
    pub fn default_on() -> Self {
        Self::Config(RawEnforceVariableUseConfig::default())
    }

    pub fn into_config(self) -> Option<RawEnforceVariableUseConfig> {
        match self {
            Self::Off => None,
            Self::Config(config) => Some(config),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawEnforceVariableUseConfig {
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default = "default_allowed_functions")]
    pub allowed_functions: Vec<String>,
    #[serde(default = "default_allowed_values")]
    pub allowed_values: Vec<String>,
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

#[derive(Debug, Clone)]
pub struct EnforceVariableUseConfig {
    pub types: Vec<SyntaxComponentKind>,
    pub allowed_functions: HashSet<String>,
    pub allowed_values: HashSet<String>,
}

impl EnforceVariableUseConfig {
    pub(crate) fn from_raw(raw: RawEnforceVariableUseConfig) -> Result<Self, ConfigError> {
        Ok(Self {
            types: raw
                .types
                .iter()
                .map(|s| parse_type_name(s))
                .collect::<Result<Vec<SyntaxComponentKind>, ConfigError>>()?,
            allowed_functions: raw.allowed_functions.iter().cloned().collect(),
            allowed_values: raw.allowed_values.iter().cloned().collect(),
        })
    }
}

fn parse_type_name(name: &str) -> Result<SyntaxComponentKind, ConfigError> {
    let r = match name {
        "color" => SyntaxComponentKind::Color,
        "length" => SyntaxComponentKind::Length,
        "number" => SyntaxComponentKind::Number,
        "percentage" => SyntaxComponentKind::Percentage,
        "length-percentage" => SyntaxComponentKind::LengthPercentage,
        "integer" => SyntaxComponentKind::Integer,
        "angle" => SyntaxComponentKind::Angle,
        "time" => SyntaxComponentKind::Time,
        "resolution" => SyntaxComponentKind::Resolution,
        "image" => SyntaxComponentKind::Image,
        "url" => SyntaxComponentKind::Url,
        "transform-function" => SyntaxComponentKind::TransformFunction,
        "transform-list" => SyntaxComponentKind::TransformList,
        _ => {
            return Err(ConfigError::InvalidRuleOption {
                raw: name.to_string(),
                reason: format!("unknown type '{name}'"),
            });
        }
    };

    Ok(r)
}
