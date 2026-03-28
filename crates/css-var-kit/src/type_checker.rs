pub mod value_kind;

use std::collections::HashMap;

use lightningcss::properties::Property as CssProperty;
use lightningcss::properties::PropertyId;
use lightningcss::properties::custom::TokenList;
use lightningcss::stylesheet::ParserOptions;
use thiserror::Error;

use crate::variable_resolver::contains_var;

#[derive(Debug, PartialEq, Error)]
pub enum TypeCheckError {
    #[error("type mismatch: resolved value of `{0}` is not valid for property `{1}`")]
    TypeMismatch(String, String),
    #[error("variable not found: {0}")]
    VariableNotFound(String),
    #[error("invalid syntax in variable declaration")]
    InvalidSyntax,
}

pub fn check_property_type(
    property_name: &str,
    value: &str,
    vars: &HashMap<&str, TokenList>,
) -> Result<(), TypeCheckError> {
    if property_name.starts_with("--") {
        return Ok(());
    }

    let property_id = PropertyId::from(property_name);
    let property = CssProperty::parse_string(property_id, value, ParserOptions::default())
        .map_err(|_| TypeCheckError::InvalidSyntax)?;

    match property {
        CssProperty::Unparsed(unparsed) => match unparsed.substitute_variables(vars) {
            Ok(CssProperty::Unparsed(result)) if contains_var(&result.value) => {
                Err(TypeCheckError::VariableNotFound(value.to_string()))
            }
            Ok(CssProperty::Unparsed(_)) => Err(TypeCheckError::TypeMismatch(
                value.to_string(),
                property_name.to_string(),
            )),
            Ok(_) => Ok(()),
            Err(()) => Err(TypeCheckError::TypeMismatch(
                value.to_string(),
                property_name.to_string(),
            )),
        },
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> HashMap<&'static str, TokenList<'static>> {
        use lightningcss::traits::ParseWithOptions;
        let mut map = HashMap::new();
        for (name, value) in [
            ("--color", "red"),
            ("--size", "16px"),
            ("--number", "42"),
            ("--invalid", "not-a-color"),
            ("--nested", "var(--color)"),
        ] {
            map.insert(
                name,
                TokenList::parse_string_with_options(value, ParserOptions::default()).unwrap(),
            );
        }
        map
    }

    #[test]
    fn valid_color() {
        assert!(check_property_type("color", "var(--color)", &vars()).is_ok());
    }

    #[test]
    fn valid_length() {
        assert!(check_property_type("font-size", "var(--size)", &vars()).is_ok());
    }

    #[test]
    fn mismatch_length_for_color() {
        assert!(matches!(
            check_property_type("color", "var(--size)", &vars()),
            Err(TypeCheckError::TypeMismatch(_, _))
        ));
    }

    #[test]
    fn mismatch_invalid_keyword() {
        assert!(matches!(
            check_property_type("color", "var(--invalid)", &vars()),
            Err(TypeCheckError::TypeMismatch(_, _))
        ));
    }

    #[test]
    fn unresolved_variable() {
        assert!(matches!(
            check_property_type("color", "var(--undefined)", &vars()),
            Err(TypeCheckError::VariableNotFound(_)),
        ));
    }

    #[test]
    fn fallback_used_when_undefined() {
        assert!(check_property_type("color", "var(--undefined, blue)", &vars()).is_ok());
    }

    #[test]
    fn custom_property_always_valid() {
        assert!(check_property_type("--my-var", "anything", &vars()).is_ok());
    }

    #[test]
    fn mixed_value() {
        assert!(check_property_type("border", "1px solid var(--color)", &vars()).is_ok());
    }

    #[test]
    fn nested_var() {
        assert!(check_property_type("color", "var(--nested)", &vars()).is_ok());
    }

    #[test]
    fn no_var_passthrough() {
        assert!(check_property_type("color", "red", &vars()).is_ok());
    }
}
