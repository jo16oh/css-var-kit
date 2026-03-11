use std::collections::HashSet;

use lightningcss::values::syntax::SyntaxComponentKind;

use crate::{
    config::ConfigError,
    rules::{
        Rule, enforce_variable_use::EnforceVariableUse,
        no_inconsistent_variable_definition::NoInconsistentVariableDefinition,
        no_undefined_variable_use::NoUndefinedVariableUse,
        no_variable_type_mismatch::NoVariableTypeMismatch,
    },
};

use super::file::{RawEnforceVariableUseConfig, RawRules};

#[derive(Debug, Clone)]
pub struct Rules {
    pub no_undefined_variable_use: bool,
    pub enforce_variable_use: Option<EnforceVariableUseConfig>,
    pub no_compound_value_in_definition: bool,
    pub no_variable_type_mismatch: bool,
    pub no_inconsistent_variable_definition: bool,
}

#[derive(Debug, Clone)]
pub struct EnforceVariableUseConfig {
    pub types: Vec<SyntaxComponentKind>,
    pub allowed_functions: HashSet<String>,
    pub allowed_values: HashSet<String>,
}

impl Rules {
    pub(in crate::config) fn from_raw(raw: RawRules) -> Result<Self, ConfigError> {
        let enforce_variable_use = raw
            .enforce_variable_use
            .into_config()
            .filter(|r| !r.types.is_empty())
            .map(EnforceVariableUseConfig::from_raw)
            .transpose()?;

        Ok(Self {
            no_undefined_variable_use: raw.no_undefined_variable_use.is_on(),
            enforce_variable_use,
            no_compound_value_in_definition: raw.no_compound_value_in_definition.is_on(),
            no_variable_type_mismatch: raw.no_variable_type_mismatch.is_on(),
            no_inconsistent_variable_definition: raw.no_inconsistent_variable_definition.is_on(),
        })
    }

    pub fn compile(&self) -> Vec<Box<dyn Rule>> {
        let mut rules: Vec<Box<dyn Rule>> = vec![];

        if self.no_undefined_variable_use {
            rules.push(Box::new(NoUndefinedVariableUse));
        }

        if let Some(ref config) = self.enforce_variable_use {
            rules.push(Box::new(EnforceVariableUse::from_config(config)));
        }

        if self.no_variable_type_mismatch {
            rules.push(Box::new(NoVariableTypeMismatch));
        }

        if self.no_inconsistent_variable_definition {
            rules.push(Box::new(NoInconsistentVariableDefinition));
        }

        rules
    }
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
