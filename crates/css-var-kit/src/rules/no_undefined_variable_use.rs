use lightningcss::properties::PropertyId;
use lightningcss::properties::custom::{TokenList, TokenOrValue};

use crate::parser::css::Property;
use crate::rules::{Diagnostic, Rule, Severity, is_ignored};
use crate::searcher::conditions::variable_definitions::VariableDefinitions;
use crate::searcher::conditions::variable_usages::VariableUsages;
use crate::searcher::{PropMapFor, SearchResult, SearcherBuilder};

const RULE_NAME: &str = "no-undefined-variable-use";

pub struct NoUndefinedVariableUse {
    pub severity: Severity,
}

impl Rule for NoUndefinedVariableUse {
    fn register_conditions<'src>(&self, searcher: SearcherBuilder<'src>) -> SearcherBuilder<'src> {
        searcher
            .add_condition(VariableDefinitions)
            .add_condition(VariableUsages)
    }

    fn check<'src>(&self, search_result: &SearchResult<'src>) -> Vec<Diagnostic<'src>> {
        let def_map = search_result.get_prop_map_for::<VariableDefinitions>();
        let usages = search_result.get_result_for(VariableUsages);
        check_undefined(&def_map, &usages, self.severity)
    }
}

fn check_undefined<'src>(
    def_map: &PropMapFor<'src, '_, VariableDefinitions>,
    usages: &[&'src Property<'src>],
    severity: Severity,
) -> Vec<Diagnostic<'src>> {
    let mut diagnostics = Vec::new();

    for prop in usages.iter() {
        if is_ignored(&prop.ignore_comments, RULE_NAME) {
            continue;
        }
        if let Some(token_list) = &prop.value.token_list {
            collect_undefined(token_list, def_map, prop, severity, &mut diagnostics);
        }
    }

    diagnostics
}

fn collect_undefined<'src>(
    token_list: &TokenList<'_>,
    definitions: &PropMapFor<'_, '_, VariableDefinitions>,
    prop: &'src Property<'src>,
    severity: Severity,
    diagnostics: &mut Vec<Diagnostic<'src>>,
) {
    for token in &token_list.0 {
        match token {
            TokenOrValue::Var(var) => {
                let name = &*var.name.ident.0;
                if !definitions.contains_key(&PropertyId::from(name)) {
                    diagnostics.push(Diagnostic {
                        file_path: prop.file_path,
                        source: prop.source,
                        line: prop.value.line,
                        column: prop.value.column,
                        span_length: None,
                        rule_name: RULE_NAME,
                        message: format!("undefined variable `{}`", name),
                        severity,
                    });
                }
                if let Some(fallback) = &var.fallback {
                    collect_undefined(fallback, definitions, prop, severity, diagnostics);
                }
            }
            TokenOrValue::DashedIdent(ident) => {
                let name = &*ident.0;
                if !definitions.contains_key(&PropertyId::from(name)) {
                    diagnostics.push(Diagnostic {
                        file_path: prop.file_path,
                        source: prop.source,
                        line: prop.value.line,
                        column: prop.value.column,
                        span_length: None,
                        rule_name: RULE_NAME,
                        message: format!("undefined variable `{}`", name),
                        severity,
                    });
                }
            }
            TokenOrValue::Function(func) => {
                collect_undefined(&func.arguments, definitions, prop, severity, diagnostics);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use std::path::Path;

    fn assert_messages(css: &str, expected: &[&str]) {
        let parse_results = [parser::css::parse(css, Path::new("test.css"))];
        let rule = NoUndefinedVariableUse {
            severity: Severity::Warning,
        };
        let searcher = rule
            .register_conditions(SearcherBuilder::new(&parse_results))
            .build();
        let search_result = searcher.search();

        let diagnostics = rule.check(&search_result);
        let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(messages, expected);
    }

    #[test]
    fn no_diagnostics_when_defined() {
        assert_messages(":root { --color: red; } .a { color: var(--color); }", &[]);
    }

    #[test]
    fn reports_undefined_variable() {
        assert_messages(
            ".a { color: var(--undefined); }",
            &["undefined variable `--undefined`"],
        );
    }

    #[test]
    fn reports_multiple_undefined() {
        assert_messages(
            ".a { background: var(--a) var(--b); }",
            &["undefined variable `--a`", "undefined variable `--b`"],
        );
    }

    #[test]
    fn mix_defined_and_undefined() {
        assert_messages(
            ":root { --color: red; } .a { color: var(--color); margin: var(--spacing); }",
            &["undefined variable `--spacing`"],
        );
    }

    #[test]
    fn nested_var_with_fallback() {
        assert_messages(
            ":root { --fallback: blue; } .a { color: var(--primary, var(--fallback)); }",
            &["undefined variable `--primary`"],
        );
    }

    #[test]
    fn no_usages_no_diagnostics() {
        assert_messages(":root { --color: red; } .a { color: red; }", &[]);
    }

    #[test]
    fn no_definitions_no_usages() {
        assert_messages(".a { color: red; }", &[]);
    }

    #[test]
    fn cvk_ignore_suppresses_warning() {
        assert_messages(
            ".a {\n    /* cvk-ignore */\n    color: var(--undefined);\n}",
            &[],
        );
    }

    #[test]
    fn cvk_ignore_only_suppresses_next_property() {
        assert_messages(
            ".a {\n    /* cvk-ignore */\n    color: var(--a);\n    margin: var(--b);\n}",
            &["undefined variable `--b`"],
        );
    }
}
