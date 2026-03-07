use super::file::{RawEnforceVariableUseConfig, RawRules};

#[derive(Debug, Clone)]
pub struct Rules {
    pub no_undefined_variable_use: bool,
    pub enforce_variable_use: Option<EnforceVariableUse>,
    pub no_compound_value_in_definition: bool,
    pub no_type_mismatch: bool,
}

#[derive(Debug, Clone)]
pub struct EnforceVariableUse {
    pub types: Vec<String>,
    pub allowed_functions: Vec<String>,
    pub allowed_values: Vec<String>,
}

impl Rules {
    pub(in crate::config) fn from_raw(raw: RawRules) -> Self {
        let enforce_variable_use = raw
            .enforce_variable_use
            .into_config()
            .filter(|r| !r.types.is_empty())
            .map(EnforceVariableUse::from_raw);

        Self {
            no_undefined_variable_use: raw.no_undefined_variable_use.is_on(),
            enforce_variable_use,
            no_compound_value_in_definition: raw.no_compound_value_in_definition.is_on(),
            no_type_mismatch: raw.no_type_mismatch.is_on(),
        }
    }
}

impl EnforceVariableUse {
    pub(in crate::config) fn from_raw(raw: RawEnforceVariableUseConfig) -> Self {
        Self {
            types: raw.types,
            allowed_functions: raw.allowed_functions,
            allowed_values: raw.allowed_values,
        }
    }
}
