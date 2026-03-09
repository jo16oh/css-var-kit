use lightningcss::values::syntax::SyntaxComponentKind;

/// A single slot within a shorthand property's value.
#[derive(Debug, PartialEq)]
pub enum Slot {
    /// A slot that expects a value matching an `@property`-compatible type.
    Typed(SyntaxComponentKind),
    /// A slot that only accepts keywords (e.g., `<line-style>`, `<display>`).
    /// These cannot be represented in `@property` syntax.
    Keyword,
}

/// Expected types for a CSS property value.
#[derive(Debug, PartialEq)]
pub enum ExpectedTypes {
    /// A property where every slot expects the same single type.
    /// Covers longhand properties and uniform shorthands (e.g., `margin`).
    Single(SyntaxComponentKind),
    /// A shorthand property with per-slot expected types.
    Shorthand(&'static [Slot]),
    /// Unknown or unsupported property.
    Unknown,
}

impl ExpectedTypes {
    /// Returns all `@property`-compatible types this property can accept,
    /// deduplicated and flattened across all slots.
    pub fn all_types(&self) -> Vec<SyntaxComponentKind> {
        match self {
            ExpectedTypes::Single(ty) => vec![ty.clone()],
            ExpectedTypes::Shorthand(slots) => {
                let mut result = Vec::new();
                for slot in *slots {
                    if let Slot::Typed(ty) = slot {
                        if !result.contains(ty) {
                            result.push(ty.clone());
                        }
                    }
                }
                result
            }
            ExpectedTypes::Unknown => vec![],
        }
    }

    /// Returns `true` if no types are known for this property.
    pub fn is_empty(&self) -> bool {
        match self {
            ExpectedTypes::Single(_) => false,
            ExpectedTypes::Shorthand(slots) => slots.iter().all(|s| matches!(s, Slot::Keyword)),
            ExpectedTypes::Unknown => true,
        }
    }
}

/// Returns the expected `@property`-compatible types for a CSS property.
///
/// - For longhand properties and uniform shorthands, returns `ExpectedTypes::Single`.
/// - For mixed-type shorthands, returns `ExpectedTypes::Shorthand` with per-slot types.
/// - Returns `ExpectedTypes::Unknown` for unknown or unsupported properties.
pub fn expected_types(property: &str) -> ExpectedTypes {
    use Slot::{Keyword, Typed};
    use SyntaxComponentKind::*;

    match property {
        // =====================================================================
        // Longhand properties / uniform shorthands
        // =====================================================================

        // <color>
        "color"
        | "background-color"
        | "border-color"
        | "border-top-color"
        | "border-right-color"
        | "border-bottom-color"
        | "border-left-color"
        | "border-block-color"
        | "border-block-start-color"
        | "border-block-end-color"
        | "border-inline-color"
        | "border-inline-start-color"
        | "border-inline-end-color"
        | "outline-color"
        | "text-decoration-color"
        | "text-emphasis-color"
        | "caret-color"
        | "accent-color"
        | "column-rule-color"
        | "flood-color"
        | "lighting-color"
        | "stop-color"
        | "fill"
        | "stroke" => ExpectedTypes::Single(Color),

        // <length>
        "border-width"
        | "border-top-width"
        | "border-right-width"
        | "border-bottom-width"
        | "border-left-width"
        | "border-block-width"
        | "border-block-start-width"
        | "border-block-end-width"
        | "border-inline-width"
        | "border-inline-start-width"
        | "border-inline-end-width"
        | "outline-width"
        | "outline-offset"
        | "column-rule-width"
        | "column-gap"
        | "row-gap"
        | "letter-spacing"
        | "word-spacing"
        | "text-indent"
        | "border-spacing"
        | "perspective"
        | "border-image-width" => ExpectedTypes::Single(Length),

        // <length-percentage>
        "width"
        | "height"
        | "min-width"
        | "min-height"
        | "max-width"
        | "max-height"
        | "inline-size"
        | "block-size"
        | "min-inline-size"
        | "min-block-size"
        | "max-inline-size"
        | "max-block-size"
        | "margin"
        | "margin-top"
        | "margin-right"
        | "margin-bottom"
        | "margin-left"
        | "margin-block"
        | "margin-block-start"
        | "margin-block-end"
        | "margin-inline"
        | "margin-inline-start"
        | "margin-inline-end"
        | "padding"
        | "padding-top"
        | "padding-right"
        | "padding-bottom"
        | "padding-left"
        | "padding-block"
        | "padding-block-start"
        | "padding-block-end"
        | "padding-inline"
        | "padding-inline-start"
        | "padding-inline-end"
        | "top"
        | "right"
        | "bottom"
        | "left"
        | "inset"
        | "inset-block"
        | "inset-block-start"
        | "inset-block-end"
        | "inset-inline"
        | "inset-inline-start"
        | "inset-inline-end"
        | "font-size"
        | "line-height"
        | "flex-basis"
        | "gap"
        | "border-radius"
        | "border-top-left-radius"
        | "border-top-right-radius"
        | "border-bottom-right-radius"
        | "border-bottom-left-radius"
        | "border-start-start-radius"
        | "border-start-end-radius"
        | "border-end-start-radius"
        | "border-end-end-radius"
        | "scroll-margin"
        | "scroll-margin-top"
        | "scroll-margin-right"
        | "scroll-margin-bottom"
        | "scroll-margin-left"
        | "scroll-padding"
        | "scroll-padding-top"
        | "scroll-padding-right"
        | "scroll-padding-bottom"
        | "scroll-padding-left"
        | "shape-margin"
        | "text-underline-offset"
        | "vertical-align" => ExpectedTypes::Single(LengthPercentage),

        // <number>
        "opacity"
        | "flex-grow"
        | "flex-shrink"
        | "order"
        | "font-size-adjust"
        | "fill-opacity"
        | "stroke-opacity"
        | "stop-opacity"
        | "flood-opacity"
        | "font-weight"
        | "line-height-step"
        | "stroke-miterlimit"
        | "tab-size" => ExpectedTypes::Single(Number),

        // <integer>
        "z-index"
        | "orphans"
        | "widows"
        | "column-count" => ExpectedTypes::Single(Integer),

        // <percentage>
        "font-stretch" => ExpectedTypes::Single(Percentage),

        // <angle>
        "rotate" => ExpectedTypes::Single(Angle),

        // <time>
        "transition-duration"
        | "transition-delay"
        | "animation-duration"
        | "animation-delay" => ExpectedTypes::Single(Time),

        // <resolution>
        "image-resolution" => ExpectedTypes::Single(Resolution),

        // <image>
        "background-image"
        | "border-image-source"
        | "list-style-image"
        | "mask-image" => ExpectedTypes::Single(Image),

        // <url>
        "cursor" => ExpectedTypes::Single(Url),

        // <transform-list>
        "transform" => ExpectedTypes::Single(TransformList),

        // =====================================================================
        // Mixed-type shorthand properties
        // =====================================================================

        // border: <line-width> || <line-style> || <color>
        "border"
        | "border-top"
        | "border-right"
        | "border-bottom"
        | "border-left"
        | "border-block"
        | "border-block-start"
        | "border-block-end"
        | "border-inline"
        | "border-inline-start"
        | "border-inline-end" => ExpectedTypes::Shorthand(&[
            Typed(Length),   // <line-width>
            Keyword,        // <line-style>
            Typed(Color),   // <color>
        ]),

        // outline: <outline-width> || <outline-style> || <outline-color>
        "outline" => ExpectedTypes::Shorthand(&[
            Typed(Length),   // <outline-width>
            Keyword,        // <outline-style>
            Typed(Color),   // <outline-color>
        ]),

        // column-rule: <column-rule-width> || <column-rule-style> || <column-rule-color>
        "column-rule" => ExpectedTypes::Shorthand(&[
            Typed(Length),   // <column-rule-width>
            Keyword,        // <column-rule-style>
            Typed(Color),   // <column-rule-color>
        ]),

        // text-decoration: <line> || <style> || <color> || <thickness>
        "text-decoration" => ExpectedTypes::Shorthand(&[
            Keyword,                  // <text-decoration-line>
            Keyword,                  // <text-decoration-style>
            Typed(Color),             // <text-decoration-color>
            Typed(LengthPercentage),  // <text-decoration-thickness>
        ]),

        // flex: <flex-grow> <flex-shrink>? <flex-basis>?
        "flex" => ExpectedTypes::Shorthand(&[
            Typed(Number),            // <flex-grow>
            Typed(Number),            // <flex-shrink>
            Typed(LengthPercentage),  // <flex-basis>
        ]),

        // list-style: <position> || <image> || <type>
        "list-style" => ExpectedTypes::Shorthand(&[
            Keyword,       // <list-style-position>
            Typed(Image),  // <list-style-image>
            Keyword,       // <list-style-type>
        ]),

        // transition: <property> <duration> <timing-function> <delay>
        "transition" => ExpectedTypes::Shorthand(&[
            Keyword,      // <property>
            Typed(Time),  // <duration>
            Keyword,      // <timing-function>
            Typed(Time),  // <delay>
        ]),

        // animation: <duration> <timing-function> <delay> <iteration-count>
        //            <direction> <fill-mode> <play-state> <name>
        "animation" => ExpectedTypes::Shorthand(&[
            Typed(Time),    // <duration>
            Keyword,        // <timing-function>
            Typed(Time),    // <delay>
            Typed(Number),  // <iteration-count>
            Keyword,        // <direction>
            Keyword,        // <fill-mode>
            Keyword,        // <play-state>
            Keyword,        // <name>
        ]),

        // background: <color> || <image> || <position> / <size> || <repeat> || <attachment> || <origin> || <clip>
        "background" => ExpectedTypes::Shorthand(&[
            Typed(Color),             // <background-color>
            Typed(Image),             // <background-image>
            Typed(LengthPercentage),  // <background-position>
            Typed(LengthPercentage),  // <background-size>
            Keyword,                  // <background-repeat>
            Keyword,                  // <background-attachment>
            Keyword,                  // <background-origin>
            Keyword,                  // <background-clip>
        ]),

        _ => ExpectedTypes::Unknown,
    }
}
