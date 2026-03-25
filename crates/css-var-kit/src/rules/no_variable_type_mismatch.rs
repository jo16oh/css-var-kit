use lightningcss::properties::PropertyId;

use crate::parser::css::Property;
use crate::rules::{Diagnostic, Rule, Severity, is_ignored};
use crate::searcher::conditions::variable_definitions::VariableDefinitions;
use crate::searcher::conditions::variable_usages::VariableUsages;
use crate::searcher::{PropMapFor, SearchResult, SearcherBuilder};
use crate::type_checker::{TypeCheckError, check_property_type};

pub struct NoVariableTypeMismatch;

impl Rule for NoVariableTypeMismatch {
    fn register_conditions<'src>(&self, searcher: SearcherBuilder<'src>) -> SearcherBuilder<'src> {
        searcher
            .add_condition(VariableDefinitions)
            .add_condition(VariableUsages)
    }

    fn check<'src>(&self, search_result: &SearchResult<'src>) -> Vec<Diagnostic<'src>> {
        let def_map = search_result.get_prop_map_for::<VariableDefinitions>();
        let usages = search_result.get_result_for(VariableUsages);
        check_type_mismatch(&def_map, &usages)
    }
}

fn check_type_mismatch<'src>(
    def_map: &PropMapFor<'src, '_, VariableDefinitions>,
    usages: &[&'src Property<'src>],
) -> Vec<Diagnostic<'src>> {
    usages
        .iter()
        .filter(|prop| !is_ignored(&prop.ignore_comments, "no-variable-type-mismatch"))
        .filter(|prop| !prop.name.raw.starts_with("--"))
        .filter_map(|prop| {
            let result = check_property_type(prop.name.raw, prop.value.raw, |name| {
                def_map
                    .get(&PropertyId::from(name))
                    .and_then(|props| props.last())
                    .map(|p| p.value.raw)
            });
            match result {
                Ok(_) => None,
                // Undefined variables are handled by no-undefined-variable-use, not this rule
                Err(TypeCheckError::VariableNotFound(_)) => None,
                Err(e) => Some(Diagnostic {
                    file_path: prop.file_path,
                    source: prop.source,
                    line: prop.value.line,
                    column: prop.value.column,
                    message: e.to_string(),
                    severity: Severity::Warning,
                }),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use crate::searcher::SearcherBuilder;
    use std::path::Path;

    fn assert_messages(css: &str, expected: &[&str]) {
        let parse_results = [parser::css::parse(css, Path::new("test.css"))];
        let rule = NoVariableTypeMismatch;
        let searcher = rule
            .register_conditions(SearcherBuilder::new(&parse_results))
            .build();
        let search_result = searcher.search();

        let diagnostics = rule.check(&search_result);
        let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(messages, expected);
    }

    #[test]
    fn valid_color_variable() {
        assert_messages(":root { --color: red; } .a { color: var(--color); }", &[]);
    }

    #[test]
    fn valid_length_variable() {
        assert_messages(
            ":root { --size: 16px; } .a { font-size: var(--size); }",
            &[],
        );
    }

    #[test]
    fn mismatch_length_for_color() {
        assert_messages(
            ":root { --size: 16px; } .a { color: var(--size); }",
            &["Type mismatch: resolved value of `var(--size)` is not valid for property `color`"],
        );
    }

    #[test]
    fn mismatch_color_for_width() {
        assert_messages(
            ":root { --color: red; } .a { width: var(--color); }",
            &["Type mismatch: resolved value of `var(--color)` is not valid for property `width`"],
        );
    }

    #[test]
    fn no_diagnostic_for_undefined_variable() {
        // Undefined variables are handled by no-undefined-variable-use, not this rule
        assert_messages(".a { color: var(--undefined); }", &[]);
    }

    #[test]
    fn valid_with_fallback() {
        assert_messages(".a { color: var(--undefined, blue); }", &[]);
    }

    #[test]
    fn mismatch_with_fallback() {
        assert_messages(
            ".a { color: var(--undefined, 16px); }",
            &[
                "Type mismatch: resolved value of `var(--undefined, 16px)` is not valid for property `color`",
            ],
        );
    }

    #[test]
    fn valid_mixed_value() {
        assert_messages(
            ":root { --color: red; } .a { border: 1px solid var(--color); }",
            &[],
        );
    }

    #[test]
    fn custom_property_definition_skipped() {
        // Custom property definitions should not be type-checked
        assert_messages(":root { --my-var: not-a-color; }", &[]);
    }

    #[test]
    fn cvk_ignore_suppresses_warning() {
        assert_messages(
            ":root { --size: 16px; } .a {\n    /* cvk-ignore */\n    color: var(--size);\n}",
            &[],
        );
    }

    #[test]
    fn cvk_ignore_with_rule_name() {
        assert_messages(
            ":root { --size: 16px; } .a {\n    /* cvk-ignore: no-variable-type-mismatch */\n    color: var(--size);\n}",
            &[],
        );
    }

    #[test]
    fn cvk_ignore_other_rule_does_not_suppress() {
        assert_messages(
            ":root { --size: 16px; } .a {\n    /* cvk-ignore: no-undefined-variable-use */\n    color: var(--size);\n}",
            &["Type mismatch: resolved value of `var(--size)` is not valid for property `color`"],
        );
    }

    #[test]
    fn valid_compound_value_in_shorthand() {
        assert_messages(
            ":root { --my-border: solid 1px black; } .a { border: var(--my-border); }",
            &[],
        );
    }

    #[test]
    fn mismatch_compound_value_in_wrong_property() {
        assert_messages(
            ":root { --my-border: solid 1px black; } .a { color: var(--my-border); }",
            &[
                "Type mismatch: resolved value of `var(--my-border)` is not valid for property `color`",
            ],
        );
    }

    #[test]
    fn multiple_usages_mixed() {
        assert_messages(
            ":root { --color: red; --size: 16px; } .a { color: var(--color); width: var(--color); }",
            &["Type mismatch: resolved value of `var(--color)` is not valid for property `width`"],
        );
    }
}
