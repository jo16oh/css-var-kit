use lightningcss::properties::custom::{TokenList, TokenOrValue};

use crate::parser::css::Property;
use crate::rules::{Diagnostic, Severity};
use crate::searcher::SearchResultFor;
use crate::searcher::conditions::variable_definitions::VariableDefinitionMap;
use crate::searcher::conditions::variable_usages::VariableUsages;

pub fn check<'src>(
    definitions: &VariableDefinitionMap<'src>,
    usages: &SearchResultFor<'src, '_, VariableUsages>,
) -> Vec<Diagnostic<'src>> {
    let mut diagnostics = Vec::new();

    for prop in usages.iter() {
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
    use crate::searcher::SearcherBuilder;
    use crate::searcher::conditions::variable_definitions::VariableDefinitions;
    use crate::searcher::conditions::variable_usages::VariableUsages;
    use std::path::Path;

    fn assert_messages(css: &str, expected: &[&str]) {
        let parse_results = [parser::css::parse(css, Path::new("test.css"))];
        let searcher = SearcherBuilder::new(&parse_results)
            .add_condition(VariableDefinitions)
            .add_condition(VariableUsages)
            .build();
        let search_result = searcher.search();

        let defs_result = search_result.get_result_for(VariableDefinitions);
        let def_map = VariableDefinitionMap::from(&defs_result);
        let usages_result = search_result.get_result_for(VariableUsages);

        let diagnostics = check(&def_map, &usages_result);
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
}
