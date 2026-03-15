include!("../../generated/value_kind_set.rs");

use lightningcss::properties::custom::{Token, TokenList, TokenOrValue};
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

        other => parsed_component_to_value_kind(other, value),
    }
}

fn parsed_component_to_value_kind(component: &ParsedComponent, raw: &str) -> ValueKind {
    match component {
        ParsedComponent::Length(_) => ValueKind::Single(ValueKindSet::LENGTH),
        ParsedComponent::Number(_) => ValueKind::Single(ValueKindSet::NUMBER),
        ParsedComponent::Percentage(_) => ValueKind::Single(ValueKindSet::PERCENTAGE),
        ParsedComponent::LengthPercentage(_) => ValueKind::Single(ValueKindSet::LENGTH_PERCENTAGE),
        ParsedComponent::Color(_) => ValueKind::Single(ValueKindSet::COLOR),
        ParsedComponent::Image(_) => ValueKind::Single(ValueKindSet::IMAGE),
        ParsedComponent::Url(_) => ValueKind::Single(ValueKindSet::URL),
        ParsedComponent::Integer(_) => ValueKind::Single(ValueKindSet::INTEGER),
        ParsedComponent::Angle(_) => ValueKind::Single(ValueKindSet::ANGLE),
        ParsedComponent::Time(_) => ValueKind::Single(ValueKindSet::TIME),
        ParsedComponent::Resolution(_) => ValueKind::Single(ValueKindSet::RESOLUTION),
        ParsedComponent::TransformFunction(_) => {
            ValueKind::Single(ValueKindSet::TRANSFORM_FUNCTION)
        }
        ParsedComponent::TransformList(_) => ValueKind::Single(ValueKindSet::TRANSFORM_LIST),
        ParsedComponent::String(_) => ValueKind::Single(ValueKindSet::STRING),
        ParsedComponent::CustomIdent(_) => ValueKind::Single(ValueKindSet::CUSTOM_IDENT),
        ParsedComponent::Literal(ident) => lookup_keyword_kinds(ident)
            .map(ValueKind::Single)
            .unwrap_or_else(|| ValueKind::Unknown(raw.to_owned())),
        ParsedComponent::TokenList(token_list) => token_list_to_value_kind(token_list, raw),
        ParsedComponent::Repeated { components, .. } => {
            let parts: Vec<ValueKindSet> = components
                .iter()
                .filter_map(|c| match parsed_component_to_value_kind(c, raw) {
                    ValueKind::Single(k) => Some(k),
                    _ => None,
                })
                .collect();

            match parts.len() {
                0 => ValueKind::Unknown(raw.to_owned()),
                1 => ValueKind::Single(parts[0]),
                _ => ValueKind::Compound(parts),
            }
        }
    }
}

fn token_list_to_value_kind(token_list: &TokenList, raw: &str) -> ValueKind {
    let parts: Vec<ValueKindSet> = token_list
        .0
        .iter()
        .map(token_or_value_to_kind_set)
        .filter(|k| !k.is_empty())
        .collect();

    match parts.len() {
        0 => ValueKind::Unknown(raw.to_owned()),
        1 => ValueKind::Single(parts[0]),
        _ => ValueKind::Compound(parts),
    }
}

fn token_or_value_to_kind_set(token: &TokenOrValue) -> ValueKindSet {
    match token {
        TokenOrValue::Color(_) | TokenOrValue::UnresolvedColor(_) => ValueKindSet::COLOR,
        TokenOrValue::Length(_) => ValueKindSet::LENGTH,
        TokenOrValue::Angle(_) => ValueKindSet::ANGLE,
        TokenOrValue::Time(_) => ValueKindSet::TIME,
        TokenOrValue::Resolution(_) => ValueKindSet::RESOLUTION,
        TokenOrValue::Url(_) => ValueKindSet::URL,
        TokenOrValue::Function(func) => {
            lookup_function_kinds(&func.name).unwrap_or(ValueKindSet::empty())
        }
        TokenOrValue::Token(tok) => raw_token_to_kind_set(tok),
        TokenOrValue::Var(_)
        | TokenOrValue::Env(_)
        | TokenOrValue::DashedIdent(_)
        | TokenOrValue::AnimationName(_) => ValueKindSet::empty(),
    }
}

fn raw_token_to_kind_set(tok: &Token) -> ValueKindSet {
    match tok {
        Token::Ident(name) => lookup_keyword_kinds(name).unwrap_or(ValueKindSet::empty()),
        Token::Number {
            int_value: Some(_), ..
        } => ValueKindSet::INTEGER | ValueKindSet::NUMBER,
        Token::Number { .. } => ValueKindSet::NUMBER,
        Token::Percentage { .. } => ValueKindSet::PERCENTAGE,
        Token::Dimension { .. }
        | Token::AtKeyword(_)
        | Token::Hash(_)
        | Token::IDHash(_)
        | Token::String(_)
        | Token::UnquotedUrl(_)
        | Token::Delim(_)
        | Token::WhiteSpace(_)
        | Token::Comment(_)
        | Token::Colon
        | Token::Semicolon
        | Token::Comma
        | Token::IncludeMatch
        | Token::DashMatch
        | Token::PrefixMatch
        | Token::SuffixMatch
        | Token::SubstringMatch
        | Token::CDO
        | Token::CDC
        | Token::Function(_)
        | Token::ParenthesisBlock
        | Token::SquareBracketBlock
        | Token::CurlyBracketBlock
        | Token::BadUrl(_)
        | Token::BadString(_)
        | Token::CloseParenthesis
        | Token::CloseSquareBracket
        | Token::CloseCurlyBracket => ValueKindSet::empty(),
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
    fn length_rem() {
        assert_single("1rem", ValueKindSet::LENGTH);
    }

    #[test]
    fn length_vw() {
        assert_single("100vw", ValueKindSet::LENGTH);
    }

    #[test]
    fn length_cm() {
        assert_single("2.5cm", ValueKindSet::LENGTH);
    }

    #[test]
    fn length_ch() {
        assert_single("10ch", ValueKindSet::LENGTH);
    }

    #[test]
    fn angle_rad() {
        assert_single("1.57rad", ValueKindSet::ANGLE);
    }

    #[test]
    fn angle_turn() {
        assert_single("0.5turn", ValueKindSet::ANGLE);
    }

    #[test]
    fn angle_grad() {
        assert_single("100grad", ValueKindSet::ANGLE);
    }

    #[test]
    fn resolution_dpcm() {
        assert_single("300dpcm", ValueKindSet::RESOLUTION);
    }

    #[test]
    fn resolution_dppx() {
        assert_single("2dppx", ValueKindSet::RESOLUTION);
    }

    #[test]
    fn unknown_dimension_unit() {
        assert!(matches!(kind_of("10foo"), ValueKind::Unknown(_)));
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
