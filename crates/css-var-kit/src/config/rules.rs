use crate::{
    config::{Config, ConfigError},
    rules::{
        Rule, Severity,
        enforce_variable_use::{EnforceVariableUse, config::EnforceVariableUseConfig},
        no_inconsistent_variable_definition::NoInconsistentVariableDefinition,
        no_undefined_variable_use::NoUndefinedVariableUse,
        no_variable_type_mismatch::NoVariableTypeMismatch,
    },
};

use super::file::RawRules;

#[derive(Debug, Clone)]
pub struct Rules {
    pub no_undefined_variable_use: Option<Severity>,
    pub enforce_variable_use: Option<EnforceVariableUseConfig>,
    pub no_variable_type_mismatch: Option<Severity>,
    pub no_inconsistent_variable_definition: Option<Severity>,
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
            no_undefined_variable_use: raw.no_undefined_variable_use.severity(),
            enforce_variable_use,
            no_variable_type_mismatch: raw.no_variable_type_mismatch.severity(),
            no_inconsistent_variable_definition: raw.no_inconsistent_variable_definition.severity(),
        };

        rules.validate_dependencies()?;

        Ok(rules)
    }

    fn validate_dependencies(&self) -> Result<(), ConfigError> {
        if self.no_variable_type_mismatch.is_some() {
            if self.no_undefined_variable_use.is_none() {
                return Err(ConfigError::MissingRuleDependency {
                    rule: "no-variable-type-mismatch",
                    dependency: "no-undefined-variable-use",
                });
            }
            if self.no_inconsistent_variable_definition.is_none() {
                return Err(ConfigError::MissingRuleDependency {
                    rule: "no-variable-type-mismatch",
                    dependency: "no-inconsistent-variable-definition",
                });
            }
        }
        Ok(())
    }

    pub fn compile(&self, config: &Config) -> Vec<Box<dyn Rule>> {
        let mut rules: Vec<Box<dyn Rule>> = vec![];

        let definition_files = &config.definition_files;
        let include = &config.include;

        if let Some(severity) = self.no_undefined_variable_use {
            rules.push(Box::new(NoUndefinedVariableUse {
                severity,
                definition_files: definition_files.clone(),
                include: include.clone(),
            }));
        }

        if let Some(ref config) = self.enforce_variable_use {
            rules.push(Box::new(EnforceVariableUse::from_config(config)));
        }

        if let Some(severity) = self.no_variable_type_mismatch {
            rules.push(Box::new(NoVariableTypeMismatch {
                severity,
                definition_files: definition_files.clone(),
                include: include.clone(),
            }));
        }

        if let Some(severity) = self.no_inconsistent_variable_definition {
            rules.push(Box::new(NoInconsistentVariableDefinition {
                severity,
                definition_files: definition_files.clone(),
                include: include.clone(),
            }));
        }

        rules
    }
}
