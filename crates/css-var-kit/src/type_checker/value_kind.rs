include!("../../generated/value_kind_set.rs");

use lightningcss::properties::custom::{Token, TokenOrValue};
use lightningcss::values::syntax::{ParsedComponent, SyntaxString};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueKind {
    Single(ValueKindSet),
    Compound(Vec<ValueKindSet>),
    Unknown(String),
}

impl ValueKind {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Single(k) => k.is_empty(),
            Self::Compound(parts) => parts.iter().all(|k| k.is_empty()),
            Self::Unknown(_) => true,
        }
    }

    pub fn is_consistent_with(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unknown(_), _) | (_, Self::Unknown(_)) => true,
            (Self::Single(a), Self::Single(b)) => a.intersects(*b),
            (Self::Compound(a), Self::Compound(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.intersects(*b))
            }
            _ => false,
        }
    }

    pub fn type_description(&self) -> String {
        match self {
            Self::Single(k) => k.iter_kind_names().collect::<Vec<_>>().join("|"),
            Self::Compound(parts) => parts
                .iter()
                .map(|k| {
                    let names = k.iter_kind_names().collect::<Vec<_>>().join("|");
                    if names.is_empty() {
                        "?".to_owned()
                    } else {
                        names
                    }
                })
                .collect::<Vec<_>>()
                .join(", "),
            Self::Unknown(raw) => format!("unknown({raw})"),
        }
    }
}
/// Returns a `ValueKind` representing the type(s) of a CSS value.
pub fn kind_of(value: &str) -> ValueKind {
    let parsed = match SyntaxString::Universal.parse_value_from_string(value) {
        Ok(p) => p,
        Err(_) => return ValueKind::Unknown(value.to_owned()),
    };

    match &parsed {
        ParsedComponent::Literal(ident) => {
            let kinds = lookup_keyword_kinds(ident).unwrap_or(ValueKindSet::empty());
            if kinds.is_empty() {
                ValueKind::Unknown(value.to_owned())
            } else {
                ValueKind::Single(kinds)
            }
        }

        ParsedComponent::TokenList(token_list) => {
            let parts: Vec<ValueKindSet> = token_list
                .0
                .iter()
                .map(token_or_value_to_kind_set)
                .filter(|k| !k.is_empty())
                .collect();

            match parts.len() {
                0 => ValueKind::Unknown(value.to_owned()),
                1 => ValueKind::Single(parts[0]),
                _ => ValueKind::Compound(parts),
            }
        }

        ParsedComponent::Repeated { components, .. } => {
            let parts: Vec<ValueKindSet> = components
                .iter()
                .map(parsed_component_to_kind_set)
                .collect();

            match parts.len() {
                0 => ValueKind::Unknown(value.to_owned()),
                1 => ValueKind::Single(parts[0]),
                _ => ValueKind::Compound(parts),
            }
        }

        other => {
            let kinds = parsed_component_to_kind_set(other);
            if kinds.is_empty() {
                ValueKind::Unknown(value.to_owned())
            } else {
                ValueKind::Single(kinds)
            }
        }
    }
}

fn token_or_value_to_kind_set(token: &TokenOrValue) -> ValueKindSet {
    match token {
        TokenOrValue::Color(_) => ValueKindSet::COLOR,
        TokenOrValue::Length(_) => ValueKindSet::LENGTH,
        TokenOrValue::Angle(_) => ValueKindSet::ANGLE,
        TokenOrValue::Time(_) => ValueKindSet::TIME,
        TokenOrValue::Resolution(_) => ValueKindSet::RESOLUTION,
        TokenOrValue::Url(_) => ValueKindSet::URL,
        TokenOrValue::Function(func) => {
            lookup_function_kinds(&func.name).unwrap_or(ValueKindSet::empty())
        }
        TokenOrValue::Token(Token::Ident(name)) => {
            lookup_keyword_kinds(name).unwrap_or(ValueKindSet::empty())
        }
        TokenOrValue::Token(Token::Number {
            int_value: Some(_), ..
        }) => ValueKindSet::INTEGER | ValueKindSet::NUMBER,
        TokenOrValue::Token(Token::Number { .. }) => ValueKindSet::NUMBER,
        TokenOrValue::Token(Token::Percentage { .. }) => ValueKindSet::PERCENTAGE,
        _ => ValueKindSet::empty(),
    }
}

fn parsed_component_to_kind_set(component: &ParsedComponent) -> ValueKindSet {
    match component {
        ParsedComponent::Length(_) => ValueKindSet::LENGTH,
        ParsedComponent::Number(_) => ValueKindSet::NUMBER,
        ParsedComponent::Percentage(_) => ValueKindSet::PERCENTAGE,
        ParsedComponent::LengthPercentage(_) => ValueKindSet::LENGTH_PERCENTAGE,
        ParsedComponent::Color(_) => ValueKindSet::COLOR,
        ParsedComponent::Image(_) => ValueKindSet::IMAGE,
        ParsedComponent::Url(_) => ValueKindSet::URL,
        ParsedComponent::Integer(_) => ValueKindSet::INTEGER,
        ParsedComponent::Angle(_) => ValueKindSet::ANGLE,
        ParsedComponent::Time(_) => ValueKindSet::TIME,
        ParsedComponent::Resolution(_) => ValueKindSet::RESOLUTION,
        ParsedComponent::TransformFunction(_) => ValueKindSet::TRANSFORM_FUNCTION,
        ParsedComponent::TransformList(_) => ValueKindSet::TRANSFORM_LIST,
        ParsedComponent::String(_) => ValueKindSet::STRING,
        ParsedComponent::CustomIdent(_) => ValueKindSet::CUSTOM_IDENT,
        _ => ValueKindSet::empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_single(value: &str, expected: ValueKindSet) {
        assert_eq!(
            kind_of(value),
            ValueKind::Single(expected),
            "kind_of({value:?})"
        );
    }

    #[test]
    fn color_keyword() {
        assert_single("red", ValueKindSet::COLOR);
    }

    #[test]
    fn hex_color() {
        assert_single("#ff0000", ValueKindSet::COLOR);
    }

    #[test]
    fn rgb_function() {
        assert_single("rgb(255, 0, 0)", ValueKindSet::COLOR);
    }

    #[test]
    fn length_px() {
        assert_single("16px", ValueKindSet::LENGTH);
    }

    #[test]
    fn length_em() {
        assert_single("2em", ValueKindSet::LENGTH);
    }

    #[test]
    fn percentage() {
        assert_single("50%", ValueKindSet::PERCENTAGE);
    }

    #[test]
    fn zero_is_number() {
        assert_single("0", ValueKindSet::INTEGER | ValueKindSet::NUMBER);
    }

    #[test]
    fn integer() {
        assert_single("42", ValueKindSet::INTEGER | ValueKindSet::NUMBER);
    }

    #[test]
    fn float_number() {
        assert_single("3.14", ValueKindSet::NUMBER);
    }

    #[test]
    fn angle_deg() {
        assert_single("90deg", ValueKindSet::ANGLE);
    }

    #[test]
    fn time_ms() {
        assert_single("300ms", ValueKindSet::TIME);
    }

    #[test]
    fn time_s() {
        assert_single("1s", ValueKindSet::TIME);
    }

    #[test]
    fn resolution() {
        assert_single("96dpi", ValueKindSet::RESOLUTION);
    }

    #[test]
    fn url_function() {
        assert_single("url(image.png)", ValueKindSet::URL);
    }

    #[test]
    fn calc_is_unknown() {
        // calc() return type depends on arguments; not statically classifiable
        assert!(matches!(
            kind_of("calc(100% - 20px)"),
            ValueKind::Unknown(_)
        ));
    }

    #[test]
    fn transparent_is_color() {
        assert_single("transparent", ValueKindSet::COLOR);
    }

    #[test]
    fn gradient_is_image() {
        assert_single("linear-gradient(red, blue)", ValueKindSet::IMAGE);
    }

    #[test]
    fn compound_border_value() {
        let result = kind_of("solid 1px black");
        assert!(matches!(result, ValueKind::Compound(_)));
        assert!(!result.is_empty());
    }

    #[test]
    fn compound_values_consistent() {
        let a = kind_of("solid 1px black");
        let b = kind_of("dashed 2px red");
        assert!(a.is_consistent_with(&b));
    }

    #[test]
    fn single_vs_compound_inconsistent() {
        let a = kind_of("red");
        let b = kind_of("solid 1px black");
        assert!(!a.is_consistent_with(&b));
    }

    #[test]
    fn unknown_value() {
        assert!(matches!(kind_of("foobar"), ValueKind::Unknown(_)));
    }

    #[test]
    fn unknown_is_consistent_with_anything() {
        let unknown = kind_of("foobar");
        let single = kind_of("red");
        assert!(unknown.is_consistent_with(&single));
        assert!(single.is_consistent_with(&unknown));
    }
}
