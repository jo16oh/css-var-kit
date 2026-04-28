use lightningcss::traits::Parse;
use lightningcss::values::color::CssColor;

pub fn parse_to_rgba(s: &str) -> Option<lsp_types::Color> {
    let parsed = CssColor::parse_string(s).ok()?;
    let rgba = match parsed.to_rgb().ok()? {
        CssColor::RGBA(rgba) => rgba,
        _ => return None,
    };
    Some(lsp_types::Color {
        red: rgba.red_f32(),
        green: rgba.green_f32(),
        blue: rgba.blue_f32(),
        alpha: rgba.alpha_f32(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1.0 / 255.0 + f32::EPSILON
    }

    fn assert_rgba(value: &str, expected: (f32, f32, f32, f32)) {
        let color = parse_to_rgba(value).unwrap_or_else(|| panic!("expected color for {value}"));
        assert!(
            approx_eq(color.red, expected.0)
                && approx_eq(color.green, expected.1)
                && approx_eq(color.blue, expected.2)
                && approx_eq(color.alpha, expected.3),
            "value={value} got=({},{},{},{}) expected={:?}",
            color.red,
            color.green,
            color.blue,
            color.alpha,
            expected,
        );
    }

    #[test]
    fn parses_hex_long() {
        assert_rgba("#ff0000", (1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn parses_hex_short() {
        assert_rgba("#f00", (1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn parses_hex_with_alpha() {
        assert_rgba("#ff000080", (1.0, 0.0, 0.0, 128.0 / 255.0));
    }

    #[test]
    fn parses_named_color() {
        assert_rgba("red", (1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn parses_transparent() {
        assert_rgba("transparent", (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn parses_rgb_function() {
        assert_rgba("rgb(255, 0, 0)", (1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn parses_rgba_function() {
        assert_rgba("rgba(255, 0, 0, 0.5)", (1.0, 0.0, 0.0, 0.5));
    }

    #[test]
    fn parses_hsl_function() {
        assert_rgba("hsl(0, 100%, 50%)", (1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn parses_oklch_function() {
        assert!(parse_to_rgba("oklch(70% 0.1 0)").is_some());
    }

    #[test]
    fn rejects_currentcolor() {
        assert!(parse_to_rgba("currentColor").is_none());
    }

    #[test]
    fn rejects_non_color_length() {
        assert!(parse_to_rgba("16px").is_none());
    }

    #[test]
    fn rejects_non_color_keyword() {
        assert!(parse_to_rgba("solid").is_none());
    }

    #[test]
    fn rejects_empty() {
        assert!(parse_to_rgba("").is_none());
    }
}
