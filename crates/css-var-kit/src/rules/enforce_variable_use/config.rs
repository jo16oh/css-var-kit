use std::collections::HashSet;

use serde::Deserialize;

use crate::config::ConfigError;
use crate::type_checker::kind_set::KindSet;

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
    pub types: KindSet,
    pub allowed_functions: HashSet<String>,
    pub allowed_values: HashSet<String>,
}

impl EnforceVariableUseConfig {
    pub(crate) fn from_raw(raw: RawEnforceVariableUseConfig) -> Result<Self, ConfigError> {
        let types = raw
            .types
            .iter()
            .map(|s| parse_type_name(s))
            .collect::<Result<Vec<KindSet>, ConfigError>>()?
            .into_iter()
            .fold(KindSet::empty(), |acc, k| acc | k);

        Ok(Self {
            types,
            allowed_functions: raw.allowed_functions.iter().cloned().collect(),
            allowed_values: raw.allowed_values.iter().cloned().collect(),
        })
    }
}

fn parse_type_name(name: &str) -> Result<KindSet, ConfigError> {
    let r = match name {
        "color" => KindSet::COLOR,
        "length" => KindSet::LENGTH,
        "number" => KindSet::NUMBER,
        "percentage" => KindSet::PERCENTAGE,
        "length-percentage" => KindSet::LENGTH_PERCENTAGE,
        "integer" => KindSet::INTEGER,
        "angle" => KindSet::ANGLE,
        "time" => KindSet::TIME,
        "resolution" => KindSet::RESOLUTION,
        "image" => KindSet::IMAGE,
        "url" => KindSet::URL,
        "transform-function" => KindSet::TRANSFORM_FUNCTION,
        "transform-list" => KindSet::TRANSFORM_LIST,
        _ => {
            return Err(ConfigError::InvalidRuleOption {
                raw: name.to_string(),
                reason: format!("unknown type '{name}'"),
            });
        }
    };

    Ok(r)
}
