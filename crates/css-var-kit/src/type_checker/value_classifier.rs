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

/// Classifies a CSS value into the set of `SyntaxComponentKind`s it satisfies.
///
/// Returns an empty `Vec` for compound values (e.g. `solid 1px black`) that
/// don't match any single type.
pub fn classify_value(value: &str) -> Vec<SyntaxComponentKind> {
    CLASSIFIABLE_KINDS
        .iter()
        .filter(|kind| try_parse_as(value, kind))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use SyntaxComponentKind::*;

    fn assert_kinds(value: &str, expected: &[SyntaxComponentKind]) {
        let result = classify_value(value);
        assert_eq!(
            result, expected,
            "classify_value({value:?}) = {result:?}, expected {expected:?}"
        );
    }

    #[test]
    fn color_keyword() {
        assert_kinds("red", &[Color]);
    }

    #[test]
    fn hex_color() {
        assert_kinds("#ff0000", &[Color]);
    }

    #[test]
    fn rgb_function() {
        assert_kinds("rgb(255, 0, 0)", &[Color]);
    }

    #[test]
    fn length_px() {
        assert_kinds("16px", &[Length, LengthPercentage]);
    }

    #[test]
    fn length_em() {
        assert_kinds("2em", &[Length, LengthPercentage]);
    }

    #[test]
    fn percentage() {
        assert_kinds("50%", &[Percentage, LengthPercentage]);
    }

    #[test]
    fn zero_is_many_types() {
        let result = classify_value("0");
        assert!(result.contains(&Length));
        assert!(result.contains(&Number));
        assert!(result.contains(&Integer));
        assert!(result.contains(&LengthPercentage));
    }

    #[test]
    fn integer() {
        assert_kinds("42", &[Length, Number, LengthPercentage, Integer]);
    }

    #[test]
    fn float_number() {
        assert_kinds("3.14", &[Length, Number, LengthPercentage]);
    }

    #[test]
    fn angle_deg() {
        assert_kinds("90deg", &[Angle]);
    }

    #[test]
    fn time_ms() {
        assert_kinds("300ms", &[Time]);
    }

    #[test]
    fn time_s() {
        assert_kinds("1s", &[Time]);
    }

    #[test]
    fn resolution() {
        assert_kinds("96dpi", &[Resolution]);
    }

    #[test]
    fn url_function() {
        assert_kinds("url(image.png)", &[Image, Url]);
    }

    #[test]
    fn compound_value_empty() {
        assert_kinds("solid 1px black", &[]);
    }

    #[test]
    fn calc_length() {
        assert_kinds("calc(100% - 20px)", &[LengthPercentage]);
    }

    #[test]
    fn transparent_is_color() {
        assert_kinds("transparent", &[Color]);
    }

    #[test]
    fn gradient_is_image() {
        assert_kinds("linear-gradient(red, blue)", &[Image]);
    }
}
