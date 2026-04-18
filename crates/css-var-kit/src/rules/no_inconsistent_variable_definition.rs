use lightningcss::properties::custom::{Token, TokenOrValue};

use crate::config::LookupFilesMatcher;
use crate::parser::css::Property;
use crate::rules::{Diagnostic, Rule, Severity, is_ignored};
use crate::searcher::conditions::variable_definitions::VariableDefinitions;
use crate::searcher::conditions::variable_definitions::VarsMap;
use crate::searcher::{SearchResult, SearcherBuilder};
use crate::type_checker::value_kind::{ValueKind, kind_of};
use crate::variable_resolver::resolve_variables;

const RULE_NAME: &str = "no-inconsistent-variable-definition";

pub struct NoInconsistentVariableDefinition {
    pub severity: Severity,
    pub definition_files: LookupFilesMatcher,
    pub include: LookupFilesMatcher,
}

impl Rule for NoInconsistentVariableDefinition {
    fn register_conditions(&self, searcher: SearcherBuilder) -> SearcherBuilder {
        searcher.add_condition(VariableDefinitions::new(
            self.definition_files.clone(),
            self.include.clone(),
        ))
    }

    fn check(&self, search_result: &SearchResult) -> Vec<Diagnostic> {
        let def_map = search_result.get_prop_map_for::<VariableDefinitions>();
        let vars = def_map.vars_map();
        let severity = self.severity;
        def_map
            .iter()
            .filter(|(_, props)| props.len() >= 2)
            .flat_map(|(_, props)| check_variable_definitions(props.as_slice(), &vars, severity))
            .collect()
    }
}

fn check_variable_definitions(
    props: &[&Property],
    vars: &VarsMap,
    severity: Severity,
) -> Vec<Diagnostic> {
    let classified: Vec<(&Property, ValueKind, bool)> = props
        .iter()
        .filter_map(|&p| {
            let token_list = p.token_list();

            // Skip check if the value is white-space only
            if token_list
                .inner()
                .0
                .iter()
                .any(|t| !matches!(t, TokenOrValue::Token(Token::WhiteSpace(_))))
            {
                let kinds = match resolve_variables(token_list.inner(), vars) {
                    Ok(resolved) => kind_of(&resolved),
                    Err(_) => kind_of(&p.value.raw),
                };
                let is_ignored = is_ignored(&p.ignore_comments, RULE_NAME);

                Some((p, kinds, is_ignored))
            } else {
                None
            }
        })
        .collect();

    if classified.len() < 2 {
        return vec![];
    }

    let (baseline_idx, baseline) = match classified
        .iter()
        .enumerate()
        .find(|(_, (_, kinds, _))| !kinds.is_empty())
    {
        Some((idx, (_, kinds, _))) => (idx, kinds),
        None => return vec![],
    };

    classified
        .iter()
        .enumerate()
        .filter(|&(idx, _)| idx != baseline_idx)
        .filter(|(_, (_, _, ignored))| !ignored)
        .filter(|(_, (_, kinds, _))| !baseline.is_consistent_with(kinds))
        .map(|(_, (prop, _, _))| {
            Diagnostic {
                file_path: prop.file_path.clone(),
                source: prop.source.clone(),
                line: prop.value.line,
                column: prop.value.column,
                span_length: None,
                rule_name: RULE_NAME,
                message: format!(
                    "inconsistent variable definition: `{}` has value `{}` which conflicts with expected type <{}>",
                    prop.ident.raw, prop.value.raw, baseline,
                ),
                severity,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned::OwnedStr;
    use crate::parser;
    use crate::searcher::SearcherBuilder;
    use std::path::PathBuf;
    use std::rc::Rc;

    fn assert_messages(css: &str, expected: &[&str]) {
        let parse_results = vec![parser::css::parse(
            OwnedStr::from(css),
            Rc::new(PathBuf::from("test.css")),
        )];
        let rule = NoInconsistentVariableDefinition {
            severity: Severity::Warning,
            definition_files: LookupFilesMatcher::default(),
            include: LookupFilesMatcher::default(),
        };
        let searcher = rule
            .register_conditions(SearcherBuilder::new(parse_results))
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
    fn consistent_color_definitions() {
        assert_messages(":root { --color: red; } .dark { --color: blue; }", &[]);
    }

    #[test]
    fn consistent_length_definitions() {
        assert_messages(":root { --size: 16px; } .large { --size: 24px; }", &[]);
    }

    #[test]
    fn inconsistent_color_vs_length() {
        assert_messages(
            ":root { --x: red; } .dark { --x: 16px; }",
            &[
                "inconsistent variable definition: `--x` has value `16px` which conflicts with expected type <color>",
            ],
        );
    }

    #[test]
    fn single_definition_skipped() {
        assert_messages(":root { --color: red; }", &[]);
    }

    #[test]
    fn skip_if_the_value_is_empty() {
        assert_messages(":root { --size: 4px;  --size:  ; }", &[]);
    }

    #[test]
    fn var_resolved_consistent() {
        assert_messages(
            ":root { --other: blue; --x: red; } .dark { --x: var(--other); }",
            &[],
        );
    }

    #[test]
    fn var_resolved_inconsistent() {
        assert_messages(
            ":root { --size: 16px; --x: red; } .dark { --x: var(--size); }",
            &[
                "inconsistent variable definition: `--x` has value `var(--size)` which conflicts with expected type <color>",
            ],
        );
    }

    #[test]
    fn var_unresolved_is_inconsistent() {
        assert_messages(
            ":root { --x: red; } .dark { --x: var(--undefined); }",
            &[
                "inconsistent variable definition: `--x` has value `var(--undefined)` which conflicts with expected type <color>",
            ],
        );
    }

    #[test]
    fn unclassifiable_vs_classifiable() {
        assert_messages(
            ":root { --x: red; } .dark { --x: solid 1px black; }",
            &[
                "inconsistent variable definition: `--x` has value `solid 1px black` which conflicts with expected type <color>",
            ],
        );
    }

    #[test]
    fn both_unclassifiable_no_warning() {
        assert_messages(
            ":root { --x: solid 1px black; } .dark { --x: dashed 2px red; }",
            &[],
        );
    }

    #[test]
    fn cvk_ignore_suppresses_warning() {
        assert_messages(
            ":root { --x: red; } .dark {\n    /* cvk-ignore */\n    --x: 16px;\n}",
            &[],
        );
    }

    #[test]
    fn cvk_ignore_with_rule_name() {
        assert_messages(
            ":root { --x: red; } .dark {\n    /* cvk-ignore: no-inconsistent-variable-definition */\n    --x: 16px;\n}",
            &[],
        );
    }

    #[test]
    fn cvk_ignore_other_rule_does_not_suppress() {
        assert_messages(
            ":root { --x: red; } .dark {\n    /* cvk-ignore: no-variable-type-mismatch */\n    --x: 16px;\n}",
            &[
                "inconsistent variable definition: `--x` has value `16px` which conflicts with expected type <color>",
            ],
        );
    }

    #[test]
    fn multiple_inconsistent_definitions() {
        assert_messages(
            ":root { --x: red; } .a { --x: 16px; } .b { --x: 300ms; }",
            &[
                "inconsistent variable definition: `--x` has value `16px` which conflicts with expected type <color>",
                "inconsistent variable definition: `--x` has value `300ms` which conflicts with expected type <color>",
            ],
        );
    }

    #[test]
    fn different_variables_independent() {
        assert_messages(
            ":root { --color: red; --size: 16px; } .dark { --color: blue; --size: 24px; }",
            &[],
        );
    }

    #[test]
    fn consistent_compound_definitions() {
        assert_messages(
            ":root { --border: solid 1px red; } .dark { --border: dashed 2px blue; }",
            &[],
        );
    }

    #[test]
    fn inconsistent_single_vs_compound() {
        assert_messages(
            ":root { --x: solid 1px red; } .dark { --x: blue; }",
            &[
                "inconsistent variable definition: `--x` has value `blue` which conflicts with expected type <leader-type|line-style, length, color>",
            ],
        );
    }
}
