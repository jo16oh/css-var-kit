pub mod value_classifier;
pub mod variable_resolver;

use lightningcss::properties::Property as CssProperty;
use lightningcss::properties::PropertyId;
use lightningcss::stylesheet::ParserOptions;

use variable_resolver::{ResolveResult, resolve_vars};

/// The result of type-checking a property value containing `var()` references.
#[derive(Debug, PartialEq)]
pub enum TypeCheckResult {
    /// The resolved value is valid for this property.
    Valid,
    /// The resolved value does not match the property's expected type.
    Mismatch,
    /// A referenced variable is undefined and has no fallback.
    Unresolved,
}

/// Resolves `var()` references in `value` using `lookup`, then validates
/// the resolved string against the property using `Property::parse_string`.
///
/// Returns `Mismatch` if lightningcss considers the resolved value unparseable
/// for the given property (i.e. `Property::Unparsed`).
pub fn check_property_type<'src>(
    property_name: &str,
    value: &'src str,
    lookup: impl Fn(&str) -> Option<&'src str>,
) -> TypeCheckResult {
    if property_name.starts_with("--") {
        return TypeCheckResult::Valid;
    }

    let resolved_value = match resolve_vars(value, &lookup) {
        ResolveResult::Resolved(s) => s,
        ResolveResult::Unresolved => return TypeCheckResult::Unresolved,
    };

    let property_id = PropertyId::from(property_name);

    match CssProperty::parse_string(property_id, &resolved_value, ParserOptions::default()) {
        Ok(CssProperty::Unparsed(_)) => TypeCheckResult::Mismatch,
        Ok(_) => TypeCheckResult::Valid,
        Err(_) => TypeCheckResult::Mismatch,
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
        assert_eq!(
            check_property_type("color", "var(--color)", lookup),
            TypeCheckResult::Valid,
        );
    }

    #[test]
    fn valid_length() {
        assert_eq!(
            check_property_type("font-size", "var(--size)", lookup),
            TypeCheckResult::Valid,
        );
    }

    #[test]
    fn mismatch_length_for_color() {
        assert_eq!(
            check_property_type("color", "var(--size)", lookup),
            TypeCheckResult::Mismatch,
        );
    }

    #[test]
    fn mismatch_invalid_keyword() {
        assert_eq!(
            check_property_type("color", "var(--invalid)", lookup),
            TypeCheckResult::Mismatch,
        );
    }

    #[test]
    fn unresolved_variable() {
        assert_eq!(
            check_property_type("color", "var(--undefined)", lookup),
            TypeCheckResult::Unresolved,
        );
    }

    #[test]
    fn fallback_used_when_undefined() {
        assert_eq!(
            check_property_type("color", "var(--undefined, blue)", lookup),
            TypeCheckResult::Valid,
        );
    }

    #[test]
    fn custom_property_always_valid() {
        assert_eq!(
            check_property_type("--my-var", "anything", lookup),
            TypeCheckResult::Valid,
        );
    }

    #[test]
    fn mixed_value() {
        assert_eq!(
            check_property_type("border", "1px solid var(--color)", lookup),
            TypeCheckResult::Valid,
        );
    }

    #[test]
    fn nested_var() {
        assert_eq!(
            check_property_type("color", "var(--nested)", lookup),
            TypeCheckResult::Valid,
        );
    }

    #[test]
    fn no_var_passthrough() {
        assert_eq!(
            check_property_type("color", "red", lookup),
            TypeCheckResult::Valid,
        );
    }
}
