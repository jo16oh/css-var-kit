use lightningcss::properties::custom::{TokenList, TokenOrValue};

use crate::parser::css::Property;
use crate::rules::{Diagnostic, Rule, Severity, is_ignored};
use crate::searcher::conditions::variable_definitions::{VariableDefinitionMap, VariableDefinitions};
use crate::searcher::conditions::variable_usages::VariableUsages;
use crate::searcher::{SearchResult, SearcherBuilder};

pub struct NoUndefinedVariableUse;

impl Rule for NoUndefinedVariableUse {
    fn register_conditions<'src>(
        &self,
        searcher: SearcherBuilder<'src>,
    ) -> SearcherBuilder<'src> {
        searcher
            .add_condition(VariableDefinitions)
            .add_condition(VariableUsages)
    }

    fn check<'src>(&self, search_result: &SearchResult<'src>) -> Vec<Diagnostic<'src>> {
        let defs = search_result.get_result_for(VariableDefinitions);
        let def_map = VariableDefinitionMap::from(&defs);
        let usages = search_result.get_result_for(VariableUsages);
        check_undefined(&def_map, &usages)
    }
}

fn check_undefined<'src>(
    definitions: &VariableDefinitionMap<'src>,
    usages: &[&'src Property<'src>],
) -> Vec<Diagnostic<'src>> {
    let mut diagnostics = Vec::new();

    for prop in usages.iter() {
        if is_ignored(&prop.ignore_comments, "no-undefined-variable-use") {
            continue;
        }
        if let Some(token_list) = &prop.value.token_list {
            collect_undefined(token_list, definitions, prop, &mut diagnostics);
        }
    }

    diagnostics
}

fn collect_undefined<'src>(
    token_list: &TokenList<'_>,
    definitions: &VariableDefinitionMap<'_>,
    prop: &'src Property<'src>,
    diagnostics: &mut Vec<Diagnostic<'src>>,
) {
    for token in &token_list.0 {
        match token {
            TokenOrValue::Var(var) => {
                let name = &*var.name.ident.0;
                if !definitions.has(name) {
                    diagnostics.push(Diagnostic {
                        file_path: prop.file_path,
                        source: prop.source,
                        line: prop.value.line,
                        column: prop.value.column,
                        message: format!("undefined variable `{}`", name),
                        severity: Severity::Warning,
                    });
                }
                if let Some(fallback) = &var.fallback {
                    collect_undefined(fallback, definitions, prop, diagnostics);
                }
            }
            TokenOrValue::DashedIdent(ident) => {
                let name = &*ident.0;
                if !definitions.has(name) {
                    diagnostics.push(Diagnostic {
                        file_path: prop.file_path,
                        source: prop.source,
                        line: prop.value.line,
                        column: prop.value.column,
                        message: format!("undefined variable `{}`", name),
                        severity: Severity::Warning,
                    });
                }
            }
            TokenOrValue::Function(func) => {
                collect_undefined(&func.arguments, definitions, prop, diagnostics);
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
        let rule = NoUndefinedVariableUse;
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
