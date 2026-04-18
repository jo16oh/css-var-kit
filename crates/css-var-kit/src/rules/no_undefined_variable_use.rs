use lightningcss::properties::custom::{TokenList, TokenOrValue};

use crate::config::LookupFilesMatcher;
use crate::owned::OwnedPropId;
use crate::parser::css::Property;
use crate::position::offset_to_position;
use crate::rules::{Diagnostic, Rule, Severity, is_ignored};
use crate::searcher::conditions::variable_definitions::VariableDefinitions;
use crate::searcher::conditions::variable_usages::VariableUsages;
use crate::searcher::{PropMapFor, SearchResult, SearcherBuilder};

const RULE_NAME: &str = "no-undefined-variable-use";

pub struct NoUndefinedVariableUse {
    pub severity: Severity,
    pub definition_files: LookupFilesMatcher,
    pub include: LookupFilesMatcher,
}

impl Rule for NoUndefinedVariableUse {
    fn register_conditions(&self, searcher: SearcherBuilder) -> SearcherBuilder {
        searcher
            .add_condition(VariableDefinitions::new(
                self.definition_files.clone(),
                self.include.clone(),
            ))
            .add_condition(VariableUsages)
    }

    fn check(&self, search_result: &SearchResult) -> Vec<Diagnostic> {
        let def_map = search_result.get_prop_map_for::<VariableDefinitions>();
        let usages = search_result.get_result_for(VariableUsages);
        check_undefined(&def_map, &usages, self.severity)
    }
}

fn check_undefined(
    def_map: &PropMapFor<'_, VariableDefinitions>,
    usages: &[Property],
    severity: Severity,
) -> Vec<Diagnostic> {
    usages
        .iter()
        .filter(|prop| !is_ignored(&prop.ignore_comments, RULE_NAME))
        .flat_map(|prop| collect_undefined(prop.token_list().inner(), def_map, prop, severity))
        .collect()
}

fn collect_undefined(
    token_list: &TokenList<'_>,
    definitions: &PropMapFor<'_, VariableDefinitions>,
    prop: &Property,
    severity: Severity,
) -> Vec<Diagnostic> {
    collect_undefined_inner(token_list, definitions, prop, severity, 0).1
}

fn collect_undefined_inner(
    token_list: &TokenList<'_>,
    definitions: &PropMapFor<'_, VariableDefinitions>,
    prop: &Property,
    severity: Severity,
    search_from: usize,
) -> (usize, Vec<Diagnostic>) {
    token_list.0.iter().fold(
        (search_from, Vec::new()),
        |(search_from, mut diagnostics), token| match token {
            TokenOrValue::Var(var) => {
                let name = &*var.name.ident.0;

                let VarPos {
                    line,
                    column,
                    span_length,
                    next_search_from,
                } = find_var_position(&prop.source, prop.value.offset, search_from, name);

                let prop_id = OwnedPropId::from(name.to_string());
                if !definitions.contains_key(&prop_id) {
                    diagnostics.push(Diagnostic {
                        file_path: prop.file_path.clone(),
                        source: prop.source.clone(),
                        line,
                        column,
                        span_length,
                        rule_name: RULE_NAME,
                        message: format!("`{}` is undefined", name),
                        severity,
                    });
                }

                let (next_sf, fallback_diags) = var
                    .fallback
                    .as_ref()
                    .map(|fb| {
                        collect_undefined_inner(fb, definitions, prop, severity, next_search_from)
                    })
                    .unwrap_or((next_search_from, Vec::new()));

                diagnostics.extend(fallback_diags);
                (next_sf, diagnostics)
            }
            TokenOrValue::DashedIdent(ident) => {
                let name = &*ident.0;

                let VarPos {
                    line,
                    column,
                    span_length,
                    next_search_from,
                } = find_var_position(&prop.source, prop.value.offset, search_from, name);

                let prop_id = OwnedPropId::from(name.to_string());
                if !definitions.contains_key(&prop_id) {
                    diagnostics.push(Diagnostic {
                        file_path: prop.file_path.clone(),
                        source: prop.source.clone(),
                        line,
                        column,
                        span_length,
                        rule_name: RULE_NAME,
                        message: format!("`{}` is undefined", name),
                        severity,
                    });
                }

                (next_search_from, diagnostics)
            }
            TokenOrValue::Function(func) => {
                let (next_search_from, inner_diagnostics) = collect_undefined_inner(
                    &func.arguments,
                    definitions,
                    prop,
                    severity,
                    search_from,
                );
                diagnostics.extend(inner_diagnostics);
                (next_search_from, diagnostics)
            }
            _ => (search_from, diagnostics),
        },
    )
}

struct VarPos {
    line: u32,
    column: u32,
    span_length: Option<u32>,
    next_search_from: usize,
}

fn find_var_position(
    source: &str,
    value_offset: usize,
    search_from: usize,
    var_name: &str,
) -> VarPos {
    let search_start = value_offset + search_from;
    source
        .get(search_start..)
        .and_then(|haystack| haystack.find(var_name))
        .map(|pos| {
            let abs_offset = search_start + pos;
            let (line, column) = offset_to_position(source, abs_offset);
            VarPos {
                line,
                column,
                span_length: Some(var_name.len() as u32),
                next_search_from: search_from + pos + var_name.len(),
            }
        })
        .unwrap_or_else(|| {
            let (line, column) = offset_to_position(source, value_offset);
            VarPos {
                line,
                column,
                span_length: None,
                next_search_from: search_from,
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned::OwnedStr;
    use crate::parser;
    use std::path::PathBuf;
    use std::rc::Rc;

    fn assert_messages(css: &str, expected: &[&str]) {
        let parse_results = vec![parser::css::parse(
            &OwnedStr::from(css),
            &Rc::new(PathBuf::from("test.css")),
        )];
        let rule = NoUndefinedVariableUse {
            severity: Severity::Warning,
            definition_files: LookupFilesMatcher::default(),
            include: LookupFilesMatcher::default(),
        };
        let searcher = rule
            .register_conditions(SearcherBuilder::new(parse_results))
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
            &["`--undefined` is undefined"],
        );
    }

    #[test]
    fn reports_multiple_undefined() {
        assert_messages(
            ".a { background: var(--a) var(--b); }",
            &["`--a` is undefined", "`--b` is undefined"],
        );
    }

    #[test]
    fn mix_defined_and_undefined() {
        assert_messages(
            ":root { --color: red; } .a { color: var(--color); margin: var(--spacing); }",
            &["`--spacing` is undefined"],
        );
    }

    #[test]
    fn nested_var_with_fallback() {
        assert_messages(
            ":root { --fallback: blue; } .a { color: var(--primary, var(--fallback)); }",
            &["`--primary` is undefined"],
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
    fn bare_double_dash_is_not_variable_usage() {
        assert_messages(".a { border: --; }", &[]);
    }

    #[test]
    fn cvk_ignore_only_suppresses_next_property() {
        assert_messages(
            ".a {\n    /* cvk-ignore */\n    color: var(--a);\n    margin: var(--b);\n}",
            &["`--b` is undefined"],
        );
    }
}
