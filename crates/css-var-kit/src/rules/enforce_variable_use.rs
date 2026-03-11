use std::collections::HashSet;

use lightningcss::properties::custom::{Token, TokenList, TokenOrValue};
use lightningcss::values::syntax::SyntaxComponentKind;

use crate::parser::css::Property;
use crate::rules::{Diagnostic, Rule, Severity, is_ignored};
use crate::searcher::SearchResult;
use crate::searcher::SearcherBuilder;
use crate::searcher::conditions::non_custom_properties::NonCustomProperties;
use crate::type_checker::value_classifier::classify_value;
use config::EnforceVariableUseConfig;

pub mod config;

const RULE_NAME: &str = "enforce-variable-use";

pub struct EnforceVariableUse {
    types: Vec<SyntaxComponentKind>,
    allowed_functions: HashSet<String>,
    allowed_values: HashSet<String>,
}

impl EnforceVariableUse {
    pub fn from_config(config: &EnforceVariableUseConfig) -> Self {
        Self {
            types: config.types.clone(),
            allowed_functions: config.allowed_functions.clone(),
            allowed_values: config.allowed_values.clone(),
        }
    }

    fn has_type(&self, kind: &SyntaxComponentKind) -> bool {
        self.types
            .iter()
            .any(|k| std::mem::discriminant(k) == std::mem::discriminant(kind))
    }
}

impl Rule for EnforceVariableUse {
    fn register_conditions<'src>(&self, searcher: SearcherBuilder<'src>) -> SearcherBuilder<'src> {
        searcher.add_condition(NonCustomProperties)
    }

    fn check<'src>(&self, search_result: &SearchResult<'src>) -> Vec<Diagnostic<'src>> {
        let props = search_result.get_result_for(NonCustomProperties);
        props
            .iter()
            .filter(|p| !is_ignored(&p.ignore_comments, RULE_NAME))
            .flat_map(|p| {
                p.value
                    .token_list
                    .as_ref()
                    .map(|tl| self.walk_tokens(tl, p))
                    .unwrap_or_default()
            })
            .collect()
    }
}

impl EnforceVariableUse {
    fn walk_tokens<'src>(
        &self,
        token_list: &TokenList<'_>,
        prop: &'src Property<'src>,
    ) -> Vec<Diagnostic<'src>> {
        token_list
            .0
            .iter()
            .flat_map(|token| match token {
                TokenOrValue::Var(_) | TokenOrValue::DashedIdent(_) => vec![],

                TokenOrValue::Function(func) => {
                    if self.allowed_functions.contains(&*func.name) {
                        vec![]
                    } else {
                        self.walk_tokens(&func.arguments, prop)
                    }
                }

                TokenOrValue::Env(_) => vec![],

                TokenOrValue::Color(_) => {
                    if self.has_type(&SyntaxComponentKind::Color) {
                        vec![make_diagnostic(prop, &SyntaxComponentKind::Color)]
                    } else {
                        vec![]
                    }
                }

                // UnresolvedColor contains var() references in color functions
                // (e.g. rgb(var(--r), 0, 0)), so it's not a pure literal
                TokenOrValue::UnresolvedColor(_) => vec![],

                TokenOrValue::Length(_) => {
                    if self.has_type(&SyntaxComponentKind::Length) {
                        vec![make_diagnostic(prop, &SyntaxComponentKind::Length)]
                    } else if self.has_type(&SyntaxComponentKind::LengthPercentage) {
                        vec![make_diagnostic(
                            prop,
                            &SyntaxComponentKind::LengthPercentage,
                        )]
                    } else {
                        vec![]
                    }
                }

                TokenOrValue::Angle(_) => {
                    if self.has_type(&SyntaxComponentKind::Angle) {
                        vec![make_diagnostic(prop, &SyntaxComponentKind::Angle)]
                    } else {
                        vec![]
                    }
                }

                TokenOrValue::Time(_) => {
                    if self.has_type(&SyntaxComponentKind::Time) {
                        vec![make_diagnostic(prop, &SyntaxComponentKind::Time)]
                    } else {
                        vec![]
                    }
                }

                TokenOrValue::Resolution(_) => {
                    if self.has_type(&SyntaxComponentKind::Resolution) {
                        vec![make_diagnostic(prop, &SyntaxComponentKind::Resolution)]
                    } else {
                        vec![]
                    }
                }

                TokenOrValue::Url(_) => {
                    if self.has_type(&SyntaxComponentKind::Url) {
                        vec![make_diagnostic(prop, &SyntaxComponentKind::Url)]
                    } else if self.has_type(&SyntaxComponentKind::Image) {
                        vec![make_diagnostic(prop, &SyntaxComponentKind::Image)]
                    } else {
                        vec![]
                    }
                }

                TokenOrValue::Token(tok) => self.check_raw_token(tok, prop),

                _ => vec![],
            })
            .collect()
    }

    fn check_raw_token<'src>(
        &self,
        tok: &Token<'_>,
        prop: &'src Property<'src>,
    ) -> Vec<Diagnostic<'src>> {
        match tok {
            Token::Percentage {
                unit_value,
                int_value,
                ..
            } => {
                if !self.has_type(&SyntaxComponentKind::Percentage)
                    && !self.has_type(&SyntaxComponentKind::LengthPercentage)
                {
                    return vec![];
                }
                let css_str = match int_value {
                    Some(i) => format!("{i}%"),
                    None => format!("{}%", unit_value * 100.0),
                };
                if is_allowed_value(&css_str, &self.allowed_values) {
                    return vec![];
                }

                if self.has_type(&SyntaxComponentKind::Percentage) {
                    vec![make_diagnostic(prop, &SyntaxComponentKind::Percentage)]
                } else {
                    vec![make_diagnostic(
                        prop,
                        &SyntaxComponentKind::LengthPercentage,
                    )]
                }
            }

            Token::Number {
                value, int_value, ..
            } => {
                let css_str = match int_value {
                    Some(i) => i.to_string(),
                    None => value.to_string(),
                };
                if is_allowed_value(&css_str, &self.allowed_values) {
                    return vec![];
                }
                // Bare numbers like `0` are also valid as lengths in CSS
                let kinds = classify_value(&css_str);
                kinds
                    .iter()
                    .find(|k| self.has_type(k))
                    .map(|k| vec![make_diagnostic(prop, k)])
                    .unwrap_or_default()
            }

            Token::Ident(s) => {
                if is_allowed_value(s, &self.allowed_values) {
                    return vec![];
                }
                let kinds = classify_value(s);
                kinds
                    .iter()
                    .find(|k| self.has_type(k))
                    .map(|k| vec![make_diagnostic(prop, k)])
                    .unwrap_or_default()
            }

            _ => vec![],
        }
    }
}

fn is_allowed_value(value: &str, allowed_values: &HashSet<String>) -> bool {
    allowed_values.contains(value)
}

fn make_diagnostic<'src>(
    prop: &'src Property<'src>,
    kind: &SyntaxComponentKind,
) -> Diagnostic<'src> {
    let kind_name = format_kind(kind);
    Diagnostic {
        file_path: prop.file_path,
        source: prop.source,
        line: prop.value.line,
        column: prop.value.column,
        message: format!(
            "use a CSS variable instead of the literal {kind_name} `{}`",
            prop.value.raw,
        ),
        severity: Severity::Warning,
    }
}

fn format_kind(kind: &SyntaxComponentKind) -> &'static str {
    match kind {
        SyntaxComponentKind::Length => "length",
        SyntaxComponentKind::Number => "number",
        SyntaxComponentKind::Percentage => "percentage",
        SyntaxComponentKind::LengthPercentage => "length-percentage",
        SyntaxComponentKind::Color => "color",
        SyntaxComponentKind::Image => "image",
        SyntaxComponentKind::Url => "url",
        SyntaxComponentKind::Integer => "integer",
        SyntaxComponentKind::Angle => "angle",
        SyntaxComponentKind::Time => "time",
        SyntaxComponentKind::Resolution => "resolution",
        SyntaxComponentKind::TransformFunction => "transform-function",
        SyntaxComponentKind::TransformList => "transform-list",
        _ => "value",
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::config::{EnforceVariableUseConfig, RawEnforceVariableUseConfig};
    use super::*;
    use crate::parser;
    use crate::searcher::SearcherBuilder;

    fn make_config(types: &[&str]) -> EnforceVariableUseConfig {
        let raw = RawEnforceVariableUseConfig {
            types: types.iter().map(|s| s.to_string()).collect(),
            allowed_functions: ["calc", "min", "max", "clamp", "env"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            allowed_values: [
                "inherit",
                "initial",
                "unset",
                "revert",
                "revert-layer",
                "currentColor",
                "transparent",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        };

        EnforceVariableUseConfig::from_raw(raw).unwrap()
    }

    fn assert_messages(css: &str, types: &[&str], expected: &[&str]) {
        let config = make_config(types);
        assert_messages_with_config(css, &config, expected);
    }

    fn assert_messages_with_config(
        css: &str,
        config: &EnforceVariableUseConfig,
        expected: &[&str],
    ) {
        let rule = EnforceVariableUse::from_config(config);
        let parse_results = [parser::css::parse(css, Path::new("test.css"))];
        let searcher = rule
            .register_conditions(SearcherBuilder::new(&parse_results))
            .build();
        let search_result = searcher.search();
        let diagnostics = rule.check(&search_result);
        let mut messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
        messages.sort();
        let mut expected = expected.to_vec();
        expected.sort();
        assert_eq!(messages, expected);
    }

    #[test]
    fn detects_literal_color() {
        assert_messages(
            ".a { color: red; }",
            &["color"],
            &["use a CSS variable instead of the literal color `red`"],
        );
    }

    #[test]
    fn allows_variable_use() {
        assert_messages(".a { color: var(--c); }", &["color"], &[]);
    }

    #[test]
    fn detects_color_in_border_shorthand() {
        assert_messages(
            ".a { border: 1px solid red; }",
            &["color"],
            &["use a CSS variable instead of the literal color `1px solid red`"],
        );
    }

    #[test]
    fn detects_color_and_length_in_border() {
        assert_messages(
            ".a { border: 1px solid red; }",
            &["color", "length"],
            &[
                "use a CSS variable instead of the literal color `1px solid red`",
                "use a CSS variable instead of the literal length `1px solid red`",
            ],
        );
    }

    #[test]
    fn allows_inherit() {
        assert_messages(".a { color: inherit; }", &["color"], &[]);
    }

    #[test]
    fn allows_transparent() {
        assert_messages(".a { color: transparent; }", &["color"], &[]);
    }

    #[test]
    fn allows_currentcolor() {
        assert_messages(".a { color: currentColor; }", &["color"], &[]);
    }

    #[test]
    fn allows_calc_function() {
        assert_messages(".a { width: calc(100% - 20px); }", &["length"], &[]);
    }

    #[test]
    fn detects_colors_in_gradient() {
        assert_messages(
            ".a { background: linear-gradient(red, blue); }",
            &["color"],
            &[
                "use a CSS variable instead of the literal color `linear-gradient(red, blue)`",
                "use a CSS variable instead of the literal color `linear-gradient(red, blue)`",
            ],
        );
    }

    #[test]
    fn skips_custom_properties() {
        assert_messages(".a { --foo: red; }", &["color"], &[]);
    }

    #[test]
    fn cvk_ignore_suppresses() {
        assert_messages(
            ".a {\n    /* cvk-ignore: enforce-variable-use */\n    color: red;\n}",
            &["color"],
            &[],
        );
    }

    #[test]
    fn cvk_ignore_bare_suppresses() {
        assert_messages(
            ".a {\n    /* cvk-ignore */\n    color: red;\n}",
            &["color"],
            &[],
        );
    }

    #[test]
    fn detects_hex_color() {
        assert_messages(
            ".a { color: #ff0000; }",
            &["color"],
            &["use a CSS variable instead of the literal color `#ff0000`"],
        );
    }

    #[test]
    fn detects_rgb_function_color() {
        assert_messages(
            ".a { color: rgb(255, 0, 0); }",
            &["color"],
            &["use a CSS variable instead of the literal color `rgb(255, 0, 0)`"],
        );
    }

    #[test]
    fn detects_length_px() {
        assert_messages(
            ".a { margin: 16px; }",
            &["length"],
            &["use a CSS variable instead of the literal length `16px`"],
        );
    }

    #[test]
    fn detects_angle() {
        assert_messages(
            ".a { transform: rotate(90deg); }",
            &["angle"],
            &["use a CSS variable instead of the literal angle `rotate(90deg)`"],
        );
    }

    #[test]
    fn detects_time() {
        assert_messages(
            ".a { transition-duration: 300ms; }",
            &["time"],
            &["use a CSS variable instead of the literal time `300ms`"],
        );
    }

    #[test]
    fn no_types_no_diagnostics() {
        assert_messages(".a { color: red; margin: 16px; }", &[], &[]);
    }

    #[test]
    fn unrelated_type_no_detection() {
        assert_messages(".a { color: red; }", &["length"], &[]);
    }

    #[test]
    fn detects_percentage() {
        assert_messages(
            ".a { width: 50%; }",
            &["percentage"],
            &["use a CSS variable instead of the literal percentage `50%`"],
        );
    }

    #[test]
    fn number_zero_detected_as_length() {
        assert_messages(
            ".a { margin: 0; }",
            &["length"],
            &["use a CSS variable instead of the literal length `0`"],
        );
    }

    #[test]
    fn env_function_skipped_by_default() {
        assert_messages(
            ".a { padding-top: env(safe-area-inset-top); }",
            &["length"],
            &[],
        );
    }

    #[test]
    fn custom_allowed_values() {
        let config = EnforceVariableUseConfig::from_raw(RawEnforceVariableUseConfig {
            types: vec!["color".to_string()],
            allowed_functions: vec![],
            allowed_values: vec!["red".to_string()],
        })
        .unwrap();
        assert_messages_with_config(
            ".a { color: red; background: blue; }",
            &config,
            &["use a CSS variable instead of the literal color `blue`"],
        );
    }

    #[test]
    fn non_allowed_function_is_traversed() {
        assert_messages(
            ".a { background: linear-gradient(red, blue); }",
            &["color"],
            &[
                "use a CSS variable instead of the literal color `linear-gradient(red, blue)`",
                "use a CSS variable instead of the literal color `linear-gradient(red, blue)`",
            ],
        );
    }

    #[test]
    fn allowed_function_is_skipped() {
        let config = EnforceVariableUseConfig::from_raw(RawEnforceVariableUseConfig {
            types: vec!["color".to_string()],
            allowed_functions: vec!["linear-gradient".to_string()],
            allowed_values: vec![],
        })
        .unwrap();
        assert_messages_with_config(
            ".a { background: linear-gradient(red, blue); }",
            &config,
            &[],
        );
    }
}
