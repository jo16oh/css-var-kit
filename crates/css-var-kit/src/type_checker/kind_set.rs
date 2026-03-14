include!("../../generated/kind_set.rs");

use cssparser::ParserInput;
use lightningcss::values::syntax::{
    Multiplier, SyntaxComponent, SyntaxComponentKind, SyntaxString,
};

/// The kinds to test against. Excludes `CustomIdent` (matches almost anything)
/// and `String` (only matches quoted strings, tested separately).
const CLASSIFIABLE_KINDS: &[SyntaxComponentKind] = &[
    SyntaxComponentKind::Length,
    SyntaxComponentKind::Number,
    SyntaxComponentKind::Percentage,
    SyntaxComponentKind::LengthPercentage,
    SyntaxComponentKind::Color,
    SyntaxComponentKind::Image,
    SyntaxComponentKind::Url,
    SyntaxComponentKind::Integer,
    SyntaxComponentKind::Angle,
    SyntaxComponentKind::Time,
    SyntaxComponentKind::Resolution,
    SyntaxComponentKind::TransformFunction,
    SyntaxComponentKind::TransformList,
];

/// Attempts to parse `value` as the given `SyntaxComponentKind`.
/// Returns `true` if parsing succeeds and consumes the entire input.
fn try_parse_as(value: &str, kind: &SyntaxComponentKind) -> bool {
    let syntax = SyntaxString::Components(vec![SyntaxComponent {
        kind: kind.clone(),
        multiplier: Multiplier::None,
    }]);

    let mut input = ParserInput::new(value);
    let mut parser = cssparser::Parser::new(&mut input);

    match syntax.parse_value(&mut parser) {
        Ok(_) => parser.is_exhausted(),
        Err(_) => false,
    }
}

/// Returns a `KindSet` representing all types the CSS value satisfies.
///
/// Returns an empty `KindSet` for compound values (e.g. `solid 1px black`) that
/// don't match any single type.
pub fn kind_of(value: &str) -> KindSet {
    let parsed = CLASSIFIABLE_KINDS
        .iter()
        .filter(|kind| try_parse_as(value, kind))
        .fold(KindSet::empty(), |acc, kind| {
            acc | from_syntax_component_kind(kind)
        });

    parsed | lookup_keyword_kinds(value).unwrap_or(KindSet::empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_keyword() {
        assert_eq!(kind_of("red"), KindSet::COLOR);
    }

    #[test]
    fn hex_color() {
        assert_eq!(kind_of("#ff0000"), KindSet::COLOR);
    }

    #[test]
    fn rgb_function() {
        assert_eq!(kind_of("rgb(255, 0, 0)"), KindSet::COLOR);
    }

    #[test]
    fn length_px() {
        assert_eq!(kind_of("16px"), KindSet::LENGTH_PERCENTAGE);
    }

    #[test]
    fn length_em() {
        assert_eq!(kind_of("2em"), KindSet::LENGTH_PERCENTAGE);
    }

    #[test]
    fn percentage() {
        assert_eq!(kind_of("50%"), KindSet::LENGTH_PERCENTAGE);
    }

    #[test]
    fn zero_is_many_types() {
        let result = kind_of("0");
        assert!(result.contains(KindSet::LENGTH));
        assert!(result.contains(KindSet::NUMBER));
        assert!(result.contains(KindSet::INTEGER));
    }

    #[test]
    fn integer() {
        assert_eq!(
            kind_of("42"),
            KindSet::LENGTH | KindSet::NUMBER | KindSet::PERCENTAGE | KindSet::INTEGER
        );
    }

    #[test]
    fn float_number() {
        assert_eq!(
            kind_of("3.14"),
            KindSet::LENGTH | KindSet::NUMBER | KindSet::PERCENTAGE
        );
    }

    #[test]
    fn angle_deg() {
        assert_eq!(kind_of("90deg"), KindSet::ANGLE);
    }

    #[test]
    fn time_ms() {
        assert_eq!(kind_of("300ms"), KindSet::TIME);
    }

    #[test]
    fn time_s() {
        assert_eq!(kind_of("1s"), KindSet::TIME);
    }

    #[test]
    fn resolution() {
        assert_eq!(kind_of("96dpi"), KindSet::RESOLUTION);
    }

    #[test]
    fn url_function() {
        assert_eq!(kind_of("url(image.png)"), KindSet::IMAGE | KindSet::URL);
    }

    #[test]
    fn compound_value_empty() {
        assert_eq!(kind_of("solid 1px black"), KindSet::empty());
    }

    #[test]
    fn calc_length() {
        assert_eq!(kind_of("calc(100% - 20px)"), KindSet::LENGTH_PERCENTAGE);
    }

    #[test]
    fn transparent_is_color() {
        assert_eq!(kind_of("transparent"), KindSet::COLOR);
    }

    #[test]
    fn gradient_is_image() {
        assert_eq!(kind_of("linear-gradient(red, blue)"), KindSet::IMAGE);
    }
}
