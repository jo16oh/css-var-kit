use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde::de::{self, Deserializer};

use crate::config::ConfigError;
use crate::config::file::SeverityToggle;
use crate::rules::Severity;
use crate::type_checker::value_kind::{ValueKindSet, lookup_kind_by_name};

#[derive(Default, Debug)]
pub enum RawEnforceVariableUse {
    #[default]
    Off,
    Config(RawEnforceVariableUseConfig),
}

impl<'de> Deserialize<'de> for RawEnforceVariableUse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        match &value {
            serde_json::Value::String(s) => match s.as_str() {
                "off" => Ok(Self::Off),
                "on" | "error" => Ok(Self::default_on()),
                "warn" => Ok(Self::default_on_with_severity(SeverityToggle::Warn)),
                _ => Err(de::Error::unknown_variant(
                    s,
                    &["off", "on", "warn", "error"],
                )),
            },
            serde_json::Value::Object(_) => serde_json::from_value(value)
                .map(Self::Config)
                .map_err(de::Error::custom),
            _ => Err(de::Error::custom("expected a string or object")),
        }
    }
}

impl RawEnforceVariableUse {
    pub fn default_on() -> Self {
        Self::Config(RawEnforceVariableUseConfig::default())
    }

    pub fn default_on_with_severity(severity: SeverityToggle) -> Self {
        Self::Config(RawEnforceVariableUseConfig {
            severity,
            ..RawEnforceVariableUseConfig::default()
        })
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
    #[serde(default = "default_severity")]
    pub severity: SeverityToggle,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default = "default_allowed_functions")]
    pub allowed_functions: Vec<String>,
    #[serde(default = "default_allowed_values")]
    pub allowed_values: Vec<String>,
    #[serde(default)]
    pub allowed_properties: Vec<RawAllowedProperty>,
}

impl Default for RawEnforceVariableUseConfig {
    fn default() -> Self {
        Self {
            severity: SeverityToggle::Error,
            types: Vec::new(),
            allowed_functions: default_allowed_functions(),
            allowed_values: default_allowed_values(),
            allowed_properties: Vec::new(),
        }
    }
}

fn default_severity() -> SeverityToggle {
    SeverityToggle::Error
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RawAllowedProperty {
    Name(String),
    WithKinds {
        #[serde(rename = "propertyName")]
        property_name: String,
        #[serde(rename = "allowedKinds")]
        allowed_kinds: Vec<String>,
    },
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
    pub severity: Severity,
    pub types: ValueKindSet,
    pub allowed_functions: HashSet<String>,
    pub allowed_values: HashSet<String>,
    pub allowed_properties: HashMap<String, ValueKindSet>,
}

impl EnforceVariableUseConfig {
    pub fn from_raw(raw: RawEnforceVariableUseConfig) -> Result<Self, ConfigError> {
        let types = raw
            .types
            .iter()
            .map(|s| parse_type_name(s))
            .collect::<Result<Vec<ValueKindSet>, ConfigError>>()?
            .into_iter()
            .fold(ValueKindSet::empty(), |acc, k| acc | k);

        let allowed_properties = raw
            .allowed_properties
            .into_iter()
            .map(|entry| match entry {
                RawAllowedProperty::Name(name) => {
                    Ok((name.to_ascii_lowercase(), ValueKindSet::all()))
                }
                RawAllowedProperty::WithKinds {
                    property_name,
                    allowed_kinds,
                } => {
                    let kinds = allowed_kinds
                        .iter()
                        .map(|s| parse_type_name(s))
                        .collect::<Result<Vec<ValueKindSet>, ConfigError>>()?
                        .into_iter()
                        .fold(ValueKindSet::empty(), |acc, k| acc | k);
                    Ok((property_name.to_ascii_lowercase(), kinds))
                }
            })
            .collect::<Result<Vec<_>, ConfigError>>()?
            .into_iter()
            .fold(HashMap::new(), |mut map, (name, kinds)| {
                *map.entry(name).or_insert(ValueKindSet::empty()) |= kinds;
                map
            });

        Ok(Self {
            severity: raw.severity.severity().unwrap_or(Severity::Warning),
            types,
            allowed_functions: raw.allowed_functions.iter().cloned().collect(),
            allowed_values: raw.allowed_values.iter().cloned().collect(),
            allowed_properties,
        })
    }
}

fn parse_type_name(name: &str) -> Result<ValueKindSet, ConfigError> {
    lookup_kind_by_name(name).ok_or_else(|| ConfigError::InvalidRuleOption {
        raw: name.to_string(),
        reason: format!("unknown type '{name}'"),
    })
}
