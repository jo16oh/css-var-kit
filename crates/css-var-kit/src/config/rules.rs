use crate::{
    config::ConfigError,
    rules::{
        Rule,
        enforce_variable_use::{EnforceVariableUse, config::EnforceVariableUseConfig},
        no_inconsistent_variable_definition::NoInconsistentVariableDefinition,
        no_undefined_variable_use::NoUndefinedVariableUse,
        no_variable_type_mismatch::NoVariableTypeMismatch,
    },
};

use super::file::RawRules;

#[derive(Debug, Clone)]
pub struct Rules {
    pub no_undefined_variable_use: bool,
    pub enforce_variable_use: Option<EnforceVariableUseConfig>,
    pub no_variable_type_mismatch: bool,
    pub no_inconsistent_variable_definition: bool,
}

impl Rules {
    pub(in crate::config) fn from_raw(raw: RawRules) -> Result<Self, ConfigError> {
        let enforce_variable_use = raw
            .enforce_variable_use
            .into_config()
            .filter(|r| !r.types.is_empty())
            .map(EnforceVariableUseConfig::from_raw)
            .transpose()?;

        let rules = Self {
            no_undefined_variable_use: raw.no_undefined_variable_use.is_on(),
            enforce_variable_use,
            no_variable_type_mismatch: raw.no_variable_type_mismatch.is_on(),
            no_inconsistent_variable_definition: raw.no_inconsistent_variable_definition.is_on(),
        };

        rules.validate_dependencies()?;

        Ok(rules)
    }

    fn validate_dependencies(&self) -> Result<(), ConfigError> {
        if self.no_variable_type_mismatch {
            if !self.no_undefined_variable_use {
                return Err(ConfigError::MissingRuleDependency {
                    rule: "no-variable-type-mismatch",
                    dependency: "no-undefined-variable-use",
                });
            }
            if !self.no_inconsistent_variable_definition {
                return Err(ConfigError::MissingRuleDependency {
                    rule: "no-variable-type-mismatch",
                    dependency: "no-inconsistent-variable-definition",
                });
            }
        }
        Ok(())
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
