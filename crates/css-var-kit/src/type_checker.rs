pub mod value_kind;
pub mod variable_resolver;

use lightningcss::properties::Property as CssProperty;
use lightningcss::properties::PropertyId;
use lightningcss::stylesheet::ParserOptions;
use thiserror::Error;

use variable_resolver::resolve_vars;

use crate::type_checker::variable_resolver::ResolveError;

#[derive(Debug, PartialEq, Error)]
pub enum TypeCheckError {
    #[error("Type mismatch: resolved value of `{0}` is not valid for property `{1}`")]
    TypeMismatch(String, String),
    #[error("Variable not found: {0}")]
    VariableNotFound(String),
    #[error("Invalid syntax in variable declaration")]
    InvalidSyntax,
}

pub fn check_property_type<'src>(
    property_name: &str,
    value: &'src str,
    lookup: impl Fn(&str) -> Option<&'src str>,
) -> Result<(), TypeCheckError> {
    // skip type-check in variable definitions
    if property_name.starts_with("--") {
        return Ok(());
    }

    let resolved_value = resolve_vars(value, &lookup).map_err(|e| match e {
        ResolveError::VariableNotFound(s) => TypeCheckError::VariableNotFound(s),
        ResolveError::InvalidSyntax => TypeCheckError::InvalidSyntax,
    })?;

    let property_id = PropertyId::from(property_name);

    match CssProperty::parse_string(property_id, &resolved_value, ParserOptions::default()) {
        Ok(CssProperty::Unparsed(_)) => Err(TypeCheckError::TypeMismatch(
            value.to_string(),
            property_name.to_string(),
        )),
        Ok(_) => Ok(()),
        Err(_) => Err(TypeCheckError::InvalidSyntax),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(name: &str) -> Option<&'static str> {
        match name {
            "--color" => Some("red"),
            "--size" => Some("16px"),
            "--number" => Some("42"),
            "--invalid" => Some("not-a-color"),
            "--nested" => Some("var(--color)"),
            _ => None,
        }
    }

    #[test]
    fn valid_color() {
        assert!(check_property_type("color", "var(--color)", lookup).is_ok());
    }

    #[test]
    fn valid_length() {
        assert!(check_property_type("font-size", "var(--size)", lookup).is_ok());
    }

    #[test]
    fn mismatch_length_for_color() {
        assert!(matches!(
            check_property_type("color", "var(--size)", lookup),
            Err(TypeCheckError::TypeMismatch(_, _))
        ));
    }

    #[test]
    fn mismatch_invalid_keyword() {
        assert!(matches!(
            check_property_type("color", "var(--invalid)", lookup),
            Err(TypeCheckError::TypeMismatch(_, _))
        ));
    }

    #[test]
    fn unresolved_variable() {
        assert!(matches!(
            check_property_type("color", "var(--undefined)", lookup),
            Err(TypeCheckError::VariableNotFound(_)),
        ));
    }

    #[test]
    fn fallback_used_when_undefined() {
        assert!(check_property_type("color", "var(--undefined, blue)", lookup).is_ok());
    }

    #[test]
    fn custom_property_always_valid() {
        assert!(check_property_type("--my-var", "anything", lookup).is_ok());
    }

    #[test]
    fn mixed_value() {
        assert!(check_property_type("border", "1px solid var(--color)", lookup).is_ok());
    }

    #[test]
    fn nested_var() {
        assert!(check_property_type("color", "var(--nested)", lookup).is_ok());
    }

    #[test]
    fn no_var_passthrough() {
        assert!(check_property_type("color", "red", lookup).is_ok());
    }
}
