use std::collections::{HashMap, HashSet};

use crate::rules::{Diagnostic, Rule, Severity, is_ignored};
use crate::searcher::Property;
use crate::searcher::SearchResult;
use crate::searcher::SearcherBuilder;
use crate::searcher::conditions::non_custom_properties::NonCustomProperties;
use crate::type_checker::value_kind::{
    ValueKindSet, lookup_dimension_unit_kinds, lookup_keyword_kinds,
};
use config::EnforceVariableUseConfig;
use cssparser::{ParseError, Parser, ParserInput, Token};

pub mod config;

const RULE_NAME: &str = "enforce-variable-use";

pub struct EnforceVariableUse {
    severity: Severity,
    types: ValueKindSet,
    allowed_functions: HashSet<String>,
    allowed_values: HashSet<String>,
    allowed_properties: HashMap<String, ValueKindSet>,
}

impl EnforceVariableUse {
    pub fn from_config(config: &EnforceVariableUseConfig) -> Self {
        Self {
            severity: config.severity,
            types: config.types,
            allowed_functions: config.allowed_functions.clone(),
            allowed_values: config.allowed_values.clone(),
            allowed_properties: config.allowed_properties.clone(),
        }
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
                let allowed_kinds = self.allowed_property_kinds(&p.ident.unescaped);
                let enforced_types = self.types & !allowed_kinds;
                if enforced_types.is_empty() {
                    return vec![];
                }
                self.check_tokens(p.value.raw, p, enforced_types)
            })
            .collect()
    }
}

impl EnforceVariableUse {
    fn allowed_property_kinds(&self, property_name: &str) -> ValueKindSet {
        self.allowed_properties
            .get(&property_name.to_ascii_lowercase())
            .copied()
            .unwrap_or(ValueKindSet::empty())
    }

    fn check_tokens<'src>(
        &self,
        raw: &str,
        prop: &Property<'src>,
        types: ValueKindSet,
    ) -> Vec<Diagnostic<'src>> {
        let mut input = ParserInput::new(raw);
        let mut parser = Parser::new(&mut input);
        self.check_tokens_inner(&mut parser, prop, types)
    }

    fn check_tokens_inner<'src>(
        &self,
        parser: &mut Parser<'_, '_>,
        prop: &Property<'src>,
        types: ValueKindSet,
    ) -> Vec<Diagnostic<'src>> {
        let mut diagnostics = Vec::new();

        loop {
            let start = parser.position();
            let token = match parser.next_including_whitespace_and_comments() {
                Ok(t) => t.clone(),
                Err(_) => break,
            };

            match token {
                Token::WhiteSpace(_)
                | Token::Comment(_)
                | Token::Comma
                | Token::Semicolon
                | Token::Delim(_) => {}

                Token::Function(ref name) => {
                    let func_name: &str = name;

                    if func_name == "var"
                        || func_name == "env"
                        || self
                            .allowed_functions
                            .contains(&func_name.to_ascii_lowercase())
                    {
                        continue;
                    }

                    let inner_diagnostics = parser
                        .parse_nested_block(
                            |inner| -> Result<Vec<Diagnostic>, ParseError<'_, ()>> {
                                Ok(self.check_tokens_inner(inner, prop, types))
                            },
                        )
                        // this function never returns Err
                        .unwrap();

                    diagnostics.extend(inner_diagnostics);
                }

                ref token => {
                    let raw = parser.slice_from(start);
                    if self.allowed_values.contains(raw) {
                        continue;
                    }
                    if let Some(diagnostic) = self.check_token(token, raw, prop, types) {
                        diagnostics.push(diagnostic);
                    }
                }
            }
        }

        diagnostics
    }

    fn check_token<'src>(
        &self,
        token: &Token<'_>,
        token_raw: &str,
        prop: &Property<'src>,
        types: ValueKindSet,
    ) -> Option<Diagnostic<'src>> {
        match token {
            Token::Hash(_) | Token::IDHash(_) => types
                .intersects(ValueKindSet::COLOR)
                .then(|| make_diagnostic(prop, token_raw, self.severity, "color")),

            Token::Dimension { unit, .. } => {
                let kinds = lookup_dimension_unit_kinds(unit)?;
                if kinds.intersects(ValueKindSet::LENGTH) {
                    if types.intersects(ValueKindSet::LENGTH) {
                        Some(make_diagnostic(prop, token_raw, self.severity, "length"))
                    } else if types.intersects(ValueKindSet::LENGTH_PERCENTAGE) {
                        Some(make_diagnostic(
                            prop,
                            token_raw,
                            self.severity,
                            "length-percentage",
                        ))
                    } else {
                        None
                    }
                } else {
                    let matched = kinds & types;
                    matched
                        .iter_kind_names()
                        .next()
                        .map(|name| make_diagnostic(prop, token_raw, self.severity, name))
                }
            }

            Token::Number { int_value, .. } => {
                let kinds = match int_value {
                    Some(_) => ValueKindSet::INTEGER | ValueKindSet::NUMBER,
                    None => ValueKindSet::NUMBER,
                };

                let matched = kinds & types;
                matched
                    .iter_kind_names()
                    .next()
                    .map(|name| make_diagnostic(prop, token_raw, self.severity, name))
            }

            Token::Percentage { .. } => {
                if types.intersects(ValueKindSet::PERCENTAGE) {
                    Some(make_diagnostic(
                        prop,
                        token_raw,
                        self.severity,
                        "percentage",
                    ))
                } else if types.intersects(ValueKindSet::LENGTH_PERCENTAGE) {
                    Some(make_diagnostic(
                        prop,
                        token_raw,
                        self.severity,
                        "length-percentage",
                    ))
                } else {
                    None
                }
            }

            Token::Ident(name) => {
                let matched = lookup_keyword_kinds(name)? & types;
                matched
                    .iter_kind_names()
                    .next()
                    .map(|name| make_diagnostic(prop, token_raw, self.severity, name))
            }

            Token::QuotedString(_) => types
                .intersects(ValueKindSet::STRING)
                .then(|| make_diagnostic(prop, token_raw, self.severity, "string")),

            Token::UnquotedUrl(_) => types
                .intersects(ValueKindSet::URL)
                .then(|| make_diagnostic(prop, token_raw, self.severity, "url")),

            _ => None,
        }
    }
}

fn make_diagnostic<'src>(
    prop: &Property<'src>,
    token_raw: &str,
    severity: Severity,
    kind_name: &str,
) -> Diagnostic<'src> {
    let token_byte_offset =
        (token_raw.as_ptr() as usize).wrapping_sub(prop.value.raw.as_ptr() as usize);
    Diagnostic {
        file_path: prop.file_path,
        source: prop.source,
        line: prop.value.line,
        column: prop.value.column + token_byte_offset as u32,
        span_length: Some(token_raw.len() as u32),
        rule_name: RULE_NAME,
        message: format!("use a CSS variable instead of the literal {kind_name} `{token_raw}`",),
        severity,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::config::{
        EnforceVariableUseConfig, RawAllowedProperty, RawEnforceVariableUseConfig,
    };
    use super::*;
    use crate::config::file::SeverityToggle;
    use crate::parser;
    use crate::searcher::SearcherBuilder;

    fn make_config(types: &[&str]) -> EnforceVariableUseConfig {
        make_config_with_allowed_properties(types, vec![])
    }

    fn make_config_with_allowed_properties(
        types: &[&str],
        allowed_properties: Vec<RawAllowedProperty>,
    ) -> EnforceVariableUseConfig {
        let raw = RawEnforceVariableUseConfig {
            severity: SeverityToggle::Warn,
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
                "#000000",
                "999px",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            allowed_properties,
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
            &["use a CSS variable instead of the literal color `red`"],
        );
    }

    #[test]
    fn detects_color_and_length_in_border() {
        assert_messages(
            ".a { border: 1px solid red; }",
            &["color", "length"],
            &[
                "use a CSS variable instead of the literal color `red`",
                "use a CSS variable instead of the literal length `1px`",
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
    fn allows_literal_color() {
        assert_messages(".a { color: #000000; }", &["color"], &[]);
    }

    #[test]
    fn allows_literal_length() {
        assert_messages(".a { width: 999px; }", &["length"], &[]);
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
                "use a CSS variable instead of the literal color `red`",
                "use a CSS variable instead of the literal color `blue`",
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
    fn rgb_arguments_are_not_colors() {
        assert_messages(".a { color: rgb(255, 0, 0); }", &["color"], &[]);
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
    fn detects_angle_inside_function() {
        assert_messages(
            ".a { transform: rotate(90deg); }",
            &["angle"],
            &["use a CSS variable instead of the literal angle `90deg`"],
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
    fn number_zero_detected_as_number() {
        assert_messages(
            ".a { margin: 0; }",
            &["number"],
            &["use a CSS variable instead of the literal number `0`"],
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
            severity: SeverityToggle::Warn,
            types: vec!["color".to_string()],
            allowed_functions: vec![],
            allowed_values: vec!["red".to_string()],
            allowed_properties: vec![],
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
                "use a CSS variable instead of the literal color `red`",
                "use a CSS variable instead of the literal color `blue`",
            ],
        );
    }

    #[test]
    fn detects_light_dark_literal_colors() {
        assert_messages(
            ".a { color: light-dark(white, black); }",
            &["color"],
            &[
                "use a CSS variable instead of the literal color `white`",
                "use a CSS variable instead of the literal color `black`",
            ],
        );
    }

    #[test]
    fn detects_light_dark_with_var() {
        assert_messages(
            ".a { color: light-dark(red, var(--dark)); }",
            &["color"],
            &["use a CSS variable instead of the literal color `red`"],
        );
    }

    #[test]
    fn rgb_with_var_alpha_no_color_detection() {
        assert_messages(
            ".a { color: rgb(255 0 0 / var(--alpha)); }",
            &["color"],
            &[],
        );
    }

    #[test]
    fn allowed_function_is_skipped() {
        let config = EnforceVariableUseConfig::from_raw(RawEnforceVariableUseConfig {
            severity: SeverityToggle::Warn,
            types: vec!["color".to_string()],
            allowed_functions: vec!["linear-gradient".to_string()],
            allowed_values: vec![],
            allowed_properties: vec![],
        })
        .unwrap();
        assert_messages_with_config(
            ".a { background: linear-gradient(red, blue); }",
            &config,
            &[],
        );
    }

    #[test]
    fn allowed_property_skips_all_checks() {
        let config = make_config_with_allowed_properties(
            &["color", "length"],
            vec![RawAllowedProperty::Name("color".to_string())],
        );
        assert_messages_with_config(
            ".a { color: red; margin: 16px; }",
            &config,
            &["use a CSS variable instead of the literal length `16px`"],
        );
    }

    #[test]
    fn allowed_property_with_kind_skips_only_that_kind() {
        let config = make_config_with_allowed_properties(
            &["color", "length"],
            vec![RawAllowedProperty::WithKinds {
                property_name: "border".to_string(),
                allowed_kinds: vec!["color".to_string()],
            }],
        );
        assert_messages_with_config(
            ".a { border: 1px solid red; }",
            &config,
            &["use a CSS variable instead of the literal length `1px`"],
        );
    }

    #[test]
    fn allowed_property_does_not_affect_other_properties() {
        let config = make_config_with_allowed_properties(
            &["color"],
            vec![RawAllowedProperty::Name("color".to_string())],
        );
        assert_messages_with_config(
            ".a { color: red; background: blue; }",
            &config,
            &["use a CSS variable instead of the literal color `blue`"],
        );
    }

    #[test]
    fn allowed_property_multiple_kinds() {
        let config = make_config_with_allowed_properties(
            &["color", "length"],
            vec![RawAllowedProperty::WithKinds {
                property_name: "border".to_string(),
                allowed_kinds: vec!["color".to_string(), "length".to_string()],
            }],
        );
        assert_messages_with_config(".a { border: 1px solid red; }", &config, &[]);
    }

    #[test]
    fn allowed_property_multiple_entries_are_ored() {
        let config = make_config_with_allowed_properties(
            &["color", "length"],
            vec![
                RawAllowedProperty::WithKinds {
                    property_name: "border".to_string(),
                    allowed_kinds: vec!["color".to_string()],
                },
                RawAllowedProperty::WithKinds {
                    property_name: "border".to_string(),
                    allowed_kinds: vec!["length".to_string()],
                },
            ],
        );
        assert_messages_with_config(".a { border: 1px solid red; }", &config, &[]);
    }

    #[test]
    fn allowed_property_is_case_insensitive() {
        let config = make_config_with_allowed_properties(
            &["color"],
            vec![RawAllowedProperty::Name("Color".to_string())],
        );
        assert_messages_with_config(".a { color: red; }", &config, &[]);
    }
}
