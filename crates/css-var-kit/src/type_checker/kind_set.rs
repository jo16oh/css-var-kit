include!("../../generated/kind_set.rs");

use lightningcss::properties::custom::{Token, TokenOrValue};
use lightningcss::values::syntax::{ParsedComponent, SyntaxString};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueKind {
    Single(KindSet),
    Compound(Vec<KindSet>),
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
            let kinds = lookup_keyword_kinds(ident).unwrap_or(KindSet::empty());
            if kinds.is_empty() {
                ValueKind::Unknown(value.to_owned())
            } else {
                ValueKind::Single(kinds)
            }
        }

        ParsedComponent::TokenList(token_list) => {
            let parts: Vec<KindSet> = token_list
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
            let parts: Vec<KindSet> = components
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

fn token_or_value_to_kind_set(token: &TokenOrValue) -> KindSet {
    match token {
        TokenOrValue::Color(_) => KindSet::COLOR,
        TokenOrValue::Length(_) => KindSet::LENGTH,
        TokenOrValue::Angle(_) => KindSet::ANGLE,
        TokenOrValue::Time(_) => KindSet::TIME,
        TokenOrValue::Resolution(_) => KindSet::RESOLUTION,
        TokenOrValue::Url(_) => KindSet::URL,
        TokenOrValue::Function(func) => {
            lookup_function_kinds(&func.name).unwrap_or(KindSet::empty())
        }
        TokenOrValue::Token(Token::Ident(name)) => {
            lookup_keyword_kinds(name).unwrap_or(KindSet::empty())
        }
        TokenOrValue::Token(Token::Number {
            int_value: Some(_), ..
        }) => KindSet::INTEGER | KindSet::NUMBER,
        TokenOrValue::Token(Token::Number { .. }) => KindSet::NUMBER,
        TokenOrValue::Token(Token::Percentage { .. }) => KindSet::PERCENTAGE,
        _ => KindSet::empty(),
    }
}

fn parsed_component_to_kind_set(component: &ParsedComponent) -> KindSet {
    match component {
        ParsedComponent::Length(_) => KindSet::LENGTH,
        ParsedComponent::Number(_) => KindSet::NUMBER,
        ParsedComponent::Percentage(_) => KindSet::PERCENTAGE,
        ParsedComponent::LengthPercentage(_) => KindSet::LENGTH_PERCENTAGE,
        ParsedComponent::Color(_) => KindSet::COLOR,
        ParsedComponent::Image(_) => KindSet::IMAGE,
        ParsedComponent::Url(_) => KindSet::URL,
        ParsedComponent::Integer(_) => KindSet::INTEGER,
        ParsedComponent::Angle(_) => KindSet::ANGLE,
        ParsedComponent::Time(_) => KindSet::TIME,
        ParsedComponent::Resolution(_) => KindSet::RESOLUTION,
        ParsedComponent::TransformFunction(_) => KindSet::TRANSFORM_FUNCTION,
        ParsedComponent::TransformList(_) => KindSet::TRANSFORM_LIST,
        ParsedComponent::String(_) => KindSet::STRING,
        ParsedComponent::CustomIdent(_) => KindSet::CUSTOM_IDENT,
        _ => KindSet::empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_single(value: &str, expected: KindSet) {
        assert_eq!(
            kind_of(value),
            ValueKind::Single(expected),
            "kind_of({value:?})"
        );
    }

    #[test]
    fn color_keyword() {
        assert_single("red", KindSet::COLOR);
    }

    #[test]
    fn hex_color() {
        assert_single("#ff0000", KindSet::COLOR);
    }

    #[test]
    fn rgb_function() {
        assert_single("rgb(255, 0, 0)", KindSet::COLOR);
    }

    #[test]
    fn length_px() {
        assert_single("16px", KindSet::LENGTH);
    }

    #[test]
    fn length_em() {
        assert_single("2em", KindSet::LENGTH);
    }

    #[test]
    fn percentage() {
        assert_single("50%", KindSet::PERCENTAGE);
    }

    #[test]
    fn zero_is_number() {
        assert_single("0", KindSet::INTEGER | KindSet::NUMBER);
    }

    #[test]
    fn integer() {
        assert_single("42", KindSet::INTEGER | KindSet::NUMBER);
    }

    #[test]
    fn float_number() {
        assert_single("3.14", KindSet::NUMBER);
    }

    #[test]
    fn angle_deg() {
        assert_single("90deg", KindSet::ANGLE);
    }

    #[test]
    fn time_ms() {
        assert_single("300ms", KindSet::TIME);
    }

    #[test]
    fn time_s() {
        assert_single("1s", KindSet::TIME);
    }

    #[test]
    fn resolution() {
        assert_single("96dpi", KindSet::RESOLUTION);
    }

    #[test]
    fn url_function() {
        assert_single("url(image.png)", KindSet::URL);
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
        assert_single("transparent", KindSet::COLOR);
    }

    #[test]
    fn gradient_is_image() {
        assert_single("linear-gradient(red, blue)", KindSet::IMAGE);
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
