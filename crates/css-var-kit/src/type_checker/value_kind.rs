include!("../../generated/value_kind_set.rs");

use lightningcss::properties::custom::{Token, TokenList, TokenOrValue};
use lightningcss::values::syntax::{
    Multiplier, ParsedComponent, SyntaxComponent, SyntaxComponentKind, SyntaxString,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueKind {
    Single(ValueKindSet),
    Compound(Vec<ValueKind>),
    Unknown(String),
}

impl ValueKind {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Single(k) => k.is_empty(),
            Self::Compound(parts) => parts.iter().all(|p| p.is_empty()),
            Self::Unknown(_) => true,
        }
    }

    pub fn is_consistent_with(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unknown(a), Self::Unknown(b)) => a == b,
            (Self::Unknown(_), _) | (_, Self::Unknown(_)) => false,
            (Self::Single(a), Self::Single(b)) => a.intersects(*b),
            (Self::Compound(a), Self::Compound(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.is_consistent_with(b))
            }
            _ => false,
        }
    }

    pub fn type_description(&self) -> String {
        match self {
            Self::Single(k) => k.iter_kind_names().collect::<Vec<_>>().join("|"),
            Self::Compound(parts) => parts
                .iter()
                .map(|p| p.type_description())
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
        Err(_) => return kind_of_unparseable(value),
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

/// Fallback for values that lightningcss cannot parse as a whole.
/// Try splitting by top-level whitespace and classifying each part independently.
fn kind_of_unparseable(value: &str) -> ValueKind {
    let chunks = split_top_level(value);
    let parts: Vec<ValueKind> = chunks.iter().map(|part| kind_of(part)).collect();

    match parts.len() {
        0 => ValueKind::Unknown(value.to_owned()),
        1 => parts[0].to_owned(),
        _ => ValueKind::Compound(parts),
    }
}

fn parsed_component_to_value_kind(component: &ParsedComponent, raw: &str) -> ValueKind {
    use ValueKind::*;

    match component {
        ParsedComponent::Length(_) => Single(ValueKindSet::LENGTH),
        ParsedComponent::Number(_) => Single(ValueKindSet::NUMBER),
        ParsedComponent::Percentage(_) => Single(ValueKindSet::PERCENTAGE),
        ParsedComponent::LengthPercentage(_) => Single(ValueKindSet::LENGTH_PERCENTAGE),
        ParsedComponent::Color(_) => Single(ValueKindSet::COLOR),
        ParsedComponent::Image(_) => Single(ValueKindSet::IMAGE),
        ParsedComponent::Url(_) => Single(ValueKindSet::URL),
        ParsedComponent::Integer(_) => Single(ValueKindSet::INTEGER),
        ParsedComponent::Angle(_) => Single(ValueKindSet::ANGLE),
        ParsedComponent::Time(_) => Single(ValueKindSet::TIME),
        ParsedComponent::Resolution(_) => Single(ValueKindSet::RESOLUTION),
        ParsedComponent::TransformFunction(_) => Single(ValueKindSet::TRANSFORM_FUNCTION),
        ParsedComponent::TransformList(_) => Single(ValueKindSet::TRANSFORM_LIST),
        ParsedComponent::String(_) => Single(ValueKindSet::STRING),
        ParsedComponent::CustomIdent(_) => Single(ValueKindSet::CUSTOM_IDENT),
        ParsedComponent::Literal(ident) => lookup_keyword_kinds(ident)
            .map(Single)
            .unwrap_or_else(|| Unknown(raw.to_owned())),
        ParsedComponent::TokenList(token_list) => token_list_to_value_kind(token_list, raw),
        ParsedComponent::Repeated { components, .. } => {
            let parts: Vec<ValueKind> = components
                .iter()
                .map(|c| parsed_component_to_value_kind(c, raw))
                .collect();

            match parts.len() {
                0 => Unknown(raw.to_owned()),
                1 => parts.into_iter().next().unwrap(),
                _ => Compound(parts),
            }
        }
    }
}

fn token_list_to_value_kind(token_list: &TokenList, raw: &str) -> ValueKind {
    let has_function = token_list
        .0
        .iter()
        .any(|t| matches!(t, TokenOrValue::Function(_)));

    if has_function {
        if let Some(kind) = try_typed_parse(raw) {
            return ValueKind::Single(kind);
        }
    }

    let chunks = split_top_level(raw);

    if chunks.len() <= 1 {
        // Single chunk: use token-level classification to avoid infinite recursion
        let parts: Vec<ValueKind> = token_list
            .0
            .iter()
            .filter_map(|t| token_or_value_to_value_kind(t))
            .collect();

        return match parts.len() {
            0 => ValueKind::Unknown(raw.to_owned()),
            1 => parts[0].to_owned(),
            _ => ValueKind::Compound(parts),
        };
    }

    let parts: Vec<ValueKind> = chunks.iter().map(|part| kind_of(part)).collect();

    match parts.len() {
        0 => ValueKind::Unknown(raw.to_owned()),
        1 => parts[0].to_owned(),
        _ => ValueKind::Compound(parts),
    }
}

/// Split a CSS value string by top-level whitespace,
/// preserving function arguments and quoted strings as single chunks.
fn split_top_level(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut in_quote: Option<char> = None;
    let mut start = 0;
    let mut has_content = false;

    for (i, c) in value.char_indices() {
        match in_quote {
            Some(q) if c == q => in_quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => in_quote = Some(c),
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = depth.saturating_sub(1),
                c if c.is_ascii_whitespace() && depth == 0 => {
                    if has_content {
                        parts.push(&value[start..i]);
                        has_content = false;
                    }
                    start = i + 1;
                    continue;
                }
                _ => {}
            },
        }
        if !has_content {
            start = i;
            has_content = true;
        }
    }

    if has_content {
        parts.push(&value[start..]);
    }

    parts
}

fn try_typed_parse(value: &str) -> Option<ValueKindSet> {
    TYPED_SYNTAX_KINDS.iter().find_map(|kind| {
        let syntax = SyntaxString::Components(vec![SyntaxComponent {
            kind: kind.clone(),
            multiplier: Multiplier::None,
        }]);
        syntax
            .parse_value_from_string(value)
            .ok()
            .map(|_| from_syntax_component_kind(kind))
    })
}

// Order matters: try more specific (unitless) types before broader ones
// so that e.g. calc(1 + 2) matches Number, not Length.
const TYPED_SYNTAX_KINDS: &[SyntaxComponentKind] = &[
    SyntaxComponentKind::Integer,
    SyntaxComponentKind::Number,
    SyntaxComponentKind::Length,
    SyntaxComponentKind::Percentage,
    SyntaxComponentKind::LengthPercentage,
    SyntaxComponentKind::Angle,
    SyntaxComponentKind::Time,
    SyntaxComponentKind::Resolution,
    SyntaxComponentKind::Color,
    SyntaxComponentKind::Image,
];

/// Returns `Some` for value-bearing tokens, `None` for structural tokens (whitespace, comments).
fn token_or_value_to_value_kind(token: &TokenOrValue) -> Option<ValueKind> {
    use ValueKind::*;

    let kind = match token {
        TokenOrValue::Color(_) | TokenOrValue::UnresolvedColor(_) => Single(ValueKindSet::COLOR),
        TokenOrValue::Length(_) => Single(ValueKindSet::LENGTH),
        TokenOrValue::Angle(_) => Single(ValueKindSet::ANGLE),
        TokenOrValue::Time(_) => Single(ValueKindSet::TIME),
        TokenOrValue::Resolution(_) => Single(ValueKindSet::RESOLUTION),
        TokenOrValue::Url(_) => Single(ValueKindSet::URL),

        TokenOrValue::Function(func) => lookup_function_kinds(&func.name)
            .map(Single)
            .unwrap_or_else(|| Unknown(func.name.to_string())),
        TokenOrValue::Token(tok) => raw_token_to_value_kind(tok)?,

        // Unclassifialbe values
        TokenOrValue::Var(v) => Unknown(format!("{v:?}")),
        TokenOrValue::Env(e) => Unknown(format!("{e:?}")),
        TokenOrValue::DashedIdent(name) => Unknown(name.0.to_string()),
        TokenOrValue::AnimationName(name) => Unknown(format!("{name:?}")),
    };

    Some(kind)
}

fn raw_token_to_value_kind(tok: &Token) -> Option<ValueKind> {
    use ValueKind::*;

    let kind = match tok {
        Token::Number {
            int_value: Some(_), ..
        } => Single(ValueKindSet::INTEGER | ValueKindSet::NUMBER),
        Token::Number { .. } => Single(ValueKindSet::NUMBER),
        Token::Percentage { .. } => Single(ValueKindSet::PERCENTAGE),
        Token::String(_) => Single(ValueKindSet::STRING),
        Token::UnquotedUrl(_) => Single(ValueKindSet::URL),

        Token::Ident(name) => lookup_keyword_kinds(name)
            .map(Single)
            .unwrap_or_else(|| Unknown(name.to_string())),
        Token::Function(name) => lookup_function_kinds(name)
            .map(Single)
            .unwrap_or_else(|| Unknown(name.to_string())),

        // Known dimension units (px, deg, ms, etc.) are already parsed into
        // TokenOrValue::Length/Angle/Time/Resolution by lightningcss.
        // A Token::Dimension here means the unit is unrecognized.
        Token::Dimension { unit, .. } => Unknown(unit.to_string()),

        // Unclassifialbe values
        Token::AtKeyword(name) => Unknown(format!("@{name}")),
        Token::Hash(s) | Token::IDHash(s) => Unknown(format!("#{s}")),
        Token::BadUrl(s) => Unknown(format!("url({s})")),
        Token::BadString(s) => Unknown(s.to_string()),
        Token::Delim(c) => Unknown(c.to_string()),
        Token::Comma => Unknown(",".to_owned()),
        Token::Colon => Unknown(":".to_owned()),
        Token::Semicolon => Unknown(";".to_owned()),
        Token::IncludeMatch => Unknown("~=".to_owned()),
        Token::DashMatch => Unknown("|=".to_owned()),
        Token::PrefixMatch => Unknown("^=".to_owned()),
        Token::SuffixMatch => Unknown("$=".to_owned()),
        Token::SubstringMatch => Unknown("*=".to_owned()),
        Token::CDO => Unknown("<!--".to_owned()),
        Token::CDC => Unknown("-->".to_owned()),
        Token::ParenthesisBlock => Unknown("(".to_owned()),
        Token::SquareBracketBlock => Unknown("[".to_owned()),
        Token::CurlyBracketBlock => Unknown("{".to_owned()),
        Token::CloseParenthesis => Unknown(")".to_owned()),
        Token::CloseSquareBracket => Unknown("]".to_owned()),
        Token::CloseCurlyBracket => Unknown("}".to_owned()),

        // Ignore whitespaces and comments
        Token::WhiteSpace(_) | Token::Comment(_) => return None,
    };

    Some(kind)
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

    // ── Basic type classification ──

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
    fn transparent_is_color() {
        assert_single("transparent", ValueKindSet::COLOR);
    }

    #[test]
    fn currentcolor_is_color() {
        assert_single("currentColor", ValueKindSet::COLOR);
    }

    #[test]
    fn hsl_is_color() {
        assert_single("hsl(0, 100%, 50%)", ValueKindSet::COLOR);
    }

    #[test]
    fn oklch_is_color() {
        assert_single("oklch(0.7 0.15 180)", ValueKindSet::COLOR);
    }

    #[test]
    fn color_mix_is_color() {
        assert_single("color-mix(in srgb, red, blue)", ValueKindSet::COLOR);
    }

    #[test]
    fn light_dark_is_color() {
        assert_single("light-dark(white, black)", ValueKindSet::COLOR);
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
    fn percentage() {
        assert_single("50%", ValueKindSet::PERCENTAGE);
    }

    #[test]
    fn integer() {
        assert_single("42", ValueKindSet::INTEGER | ValueKindSet::NUMBER);
    }

    #[test]
    fn zero_is_integer() {
        assert_single("0", ValueKindSet::INTEGER | ValueKindSet::NUMBER);
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
    fn time_ms() {
        assert_single("300ms", ValueKindSet::TIME);
    }

    #[test]
    fn time_s() {
        assert_single("1s", ValueKindSet::TIME);
    }

    #[test]
    fn resolution_dpi() {
        assert_single("96dpi", ValueKindSet::RESOLUTION);
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
    fn url_function() {
        assert_single("url(image.png)", ValueKindSet::URL);
    }

    #[test]
    fn url_with_quotes() {
        assert_single("url('image.png')", ValueKindSet::URL);
    }

    #[test]
    fn string_double_quoted() {
        assert_single("\"hello world\"", ValueKindSet::STRING);
    }

    #[test]
    fn string_single_quoted() {
        assert_single("'hello world'", ValueKindSet::STRING);
    }

    #[test]
    fn gradient_is_image() {
        assert_single("linear-gradient(red, blue)", ValueKindSet::IMAGE);
    }

    // ── Case insensitivity ──

    #[test]
    fn uppercase_color_keyword() {
        assert_single("RED", ValueKindSet::COLOR);
    }

    #[test]
    fn mixed_case_color_keyword() {
        assert_single("ReD", ValueKindSet::COLOR);
    }

    #[test]
    fn uppercase_transparent() {
        assert_single("TRANSPARENT", ValueKindSet::COLOR);
    }

    #[test]
    fn uppercase_hex_color() {
        assert_single("#FF0000", ValueKindSet::COLOR);
    }

    #[test]
    fn uppercase_unit() {
        assert_single("16PX", ValueKindSet::LENGTH);
    }

    #[test]
    fn uppercase_rgb_function() {
        assert_single("RGB(255, 0, 0)", ValueKindSet::COLOR);
    }

    #[test]
    fn currentcolor_case_insensitive() {
        assert_single("currentcolor", ValueKindSet::COLOR);
        assert_single("CURRENTCOLOR", ValueKindSet::COLOR);
    }

    // ── Signed values ──

    #[test]
    fn negative_length() {
        assert_single("-10px", ValueKindSet::LENGTH);
    }

    #[test]
    fn positive_prefix_length() {
        assert_single("+10px", ValueKindSet::LENGTH);
    }

    #[test]
    fn negative_zero() {
        assert_single("-0", ValueKindSet::INTEGER | ValueKindSet::NUMBER);
    }

    #[test]
    fn negative_percentage() {
        assert_single("-50%", ValueKindSet::PERCENTAGE);
    }

    #[test]
    fn negative_angle() {
        assert_single("-90deg", ValueKindSet::ANGLE);
    }

    // ── Zero with/without unit ──

    #[test]
    fn zero_px_is_length() {
        assert_single("0px", ValueKindSet::LENGTH);
    }

    #[test]
    fn zero_without_unit_is_number() {
        assert_single("0", ValueKindSet::INTEGER | ValueKindSet::NUMBER);
    }

    // ── Empty / whitespace / comment ──

    #[test]
    fn empty_string_is_empty() {
        assert!(kind_of("").is_empty());
    }

    #[test]
    fn whitespace_only_is_empty() {
        assert!(kind_of("   ").is_empty());
    }

    #[test]
    fn comment_only_is_empty() {
        assert!(kind_of("/* hello */").is_empty());
    }

    #[test]
    fn leading_trailing_whitespace() {
        assert_single("  red  ", ValueKindSet::COLOR);
    }

    // ── Unknown values ──

    #[test]
    fn unknown_keyword() {
        assert!(matches!(kind_of("foobar"), ValueKind::Unknown(_)));
    }

    #[test]
    fn unknown_dimension_unit() {
        assert!(matches!(kind_of("10foo"), ValueKind::Unknown(_)));
    }

    #[test]
    fn unknown_function() {
        assert!(matches!(
            kind_of("unknown-func(10px)"),
            ValueKind::Unknown(_)
        ));
    }

    #[test]
    fn inherit_is_unknown() {
        assert!(matches!(kind_of("inherit"), ValueKind::Unknown(_)));
    }

    #[test]
    fn initial_is_unknown() {
        assert!(matches!(kind_of("initial"), ValueKind::Unknown(_)));
    }

    // ── Multi-context keywords ──

    #[test]
    fn none_has_multiple_kinds() {
        let result = kind_of("none");
        if let ValueKind::Single(kinds) = &result {
            assert!(kinds.iter_kind_names().count() > 1, "got {kinds:?}");
        } else {
            panic!("expected Single, got {result:?}");
        }
    }

    #[test]
    fn auto_has_multiple_kinds() {
        let result = kind_of("auto");
        if let ValueKind::Single(kinds) = &result {
            assert!(kinds.iter_kind_names().count() > 1, "got {kinds:?}");
        } else {
            panic!("expected Single, got {result:?}");
        }
    }

    // ── calc() / math functions ──

    #[test]
    fn calc_length() {
        assert_single("calc(10px + 20px)", ValueKindSet::LENGTH);
    }

    #[test]
    fn calc_length_percentage() {
        assert_single("calc(100% - 20px)", ValueKindSet::LENGTH_PERCENTAGE);
    }

    #[test]
    fn calc_angle() {
        assert_single("calc(90deg + 10deg)", ValueKindSet::ANGLE);
    }

    #[test]
    fn calc_time() {
        assert_single("calc(1s + 500ms)", ValueKindSet::TIME);
    }

    #[test]
    fn calc_pure_number() {
        let result = kind_of("calc(1 + 2)");
        assert!(
            matches!(&result, ValueKind::Single(k) if k.intersects(ValueKindSet::NUMBER)),
            "expected NUMBER-like, got {result:?}"
        );
    }

    #[test]
    fn nested_calc() {
        assert_single("calc(calc(10px + 5px) + 5px)", ValueKindSet::LENGTH);
    }

    #[test]
    fn min_length() {
        assert_single("min(10px, 20px)", ValueKindSet::LENGTH);
    }

    #[test]
    fn max_length() {
        assert_single("max(10px, 20px)", ValueKindSet::LENGTH);
    }

    #[test]
    fn clamp_length() {
        assert_single("clamp(10px, 5vw, 100px)", ValueKindSet::LENGTH);
    }

    #[test]
    fn min_length_percentage() {
        assert_single("min(10px, 50%)", ValueKindSet::LENGTH_PERCENTAGE);
    }

    // ── is_consistent_with — simple ──

    #[test]
    fn integer_consistent_with_float() {
        // 42 is INTEGER|NUMBER, 3.14 is NUMBER — bits overlap
        assert!(kind_of("42").is_consistent_with(&kind_of("3.14")));
    }

    #[test]
    fn same_unknown_is_consistent() {
        assert!(kind_of("foobar").is_consistent_with(&kind_of("foobar")));
    }

    #[test]
    fn different_unknown_is_inconsistent() {
        assert!(!kind_of("foobar").is_consistent_with(&kind_of("bazqux")));
    }

    #[test]
    fn unknown_inconsistent_with_known() {
        assert!(!kind_of("foobar").is_consistent_with(&kind_of("red")));
        assert!(!kind_of("red").is_consistent_with(&kind_of("foobar")));
    }

    #[test]
    fn none_consistent_with_none() {
        assert!(kind_of("none").is_consistent_with(&kind_of("none")));
    }

    #[test]
    fn css_wide_keywords_self_consistent() {
        assert!(kind_of("inherit").is_consistent_with(&kind_of("inherit")));
    }

    #[test]
    fn different_css_wide_keywords_inconsistent() {
        assert!(!kind_of("inherit").is_consistent_with(&kind_of("initial")));
    }

    #[test]
    fn consistency_is_symmetric() {
        let pairs = [
            ("red", "blue"),
            ("16px", "24px"),
            ("red", "16px"),
            ("solid 1px black", "red"),
            ("foobar", "red"),
        ];
        for (a, b) in pairs {
            let ka = kind_of(a);
            let kb = kind_of(b);
            assert_eq!(
                ka.is_consistent_with(&kb),
                kb.is_consistent_with(&ka),
                "symmetry violated for ({a:?}, {b:?})"
            );
        }
    }

    // ── Compound values ──

    #[test]
    fn compound_border_value() {
        assert!(matches!(kind_of("solid 1px black"), ValueKind::Compound(_)));
    }

    #[test]
    fn compound_values_consistent() {
        assert!(kind_of("solid 1px black").is_consistent_with(&kind_of("dashed 2px red")));
    }

    #[test]
    fn single_vs_compound_inconsistent() {
        assert!(!kind_of("red").is_consistent_with(&kind_of("solid 1px black")));
    }

    #[test]
    fn compound_different_length_inconsistent() {
        assert!(!kind_of("1px 2px").is_consistent_with(&kind_of("1px 2px 3px")));
    }

    #[test]
    fn compound_same_types_different_values_consistent() {
        assert!(kind_of("1px 2px").is_consistent_with(&kind_of("10em 20rem")));
    }

    #[test]
    fn extra_spaces_in_compound() {
        assert!(kind_of("solid 1px black").is_consistent_with(&kind_of("solid  1px  black")));
    }

    #[test]
    fn compound_with_min_function() {
        let result = kind_of("solid min(1px, 2px) red");
        let expected = kind_of("dashed 10px blue");
        assert!(result.is_consistent_with(&expected));
    }

    #[test]
    fn compound_with_nested_calc_in_min() {
        let result = kind_of("solid min(calc(1px + 2px), 10px) red");
        let expected = kind_of("dashed 10px blue");
        assert!(result.is_consistent_with(&expected));
    }

    #[test]
    fn quoted_string_with_space_in_compound() {
        // Spaces inside quotes should not split the string into separate parts
        let result = kind_of("\"hello world\" red");
        if let ValueKind::Compound(parts) = &result {
            assert_eq!(parts.len(), 2, "expected 2 parts, got {parts:?}");
        } else {
            panic!("expected Compound, got {result:?}");
        }
    }

    // ── Structural edge cases ──

    #[test]
    fn slash_delimiter_changes_structure() {
        assert!(!kind_of("16px/1.5").is_consistent_with(&kind_of("16px 1.5")));
    }

    #[test]
    fn var_with_fallback_differs_from_without() {
        assert!(!kind_of("var(--foo, red)").is_consistent_with(&kind_of("var(--foo)")));
    }

    #[test]
    fn env_different_reference_is_inconsistent() {
        assert!(
            !kind_of("env(safe-area-inset-top)")
                .is_consistent_with(&kind_of("env(safe-area-inset-bottom)"))
        );
    }

    #[test]
    fn attr_match_operators_in_value() {
        for op in ["~=", "|=", "^=", "$=", "*="] {
            let value = format!("foo {op} bar");
            let result = kind_of(&value);
            if let ValueKind::Compound(parts) = &result {
                assert!(
                    parts.len() >= 3,
                    "{op}: expected at least 3 parts, got {parts:?}"
                );
            } else {
                panic!("{op}: expected Compound, got {result:?}");
            }
        }
    }
}
