use lightningcss::properties::custom::{TokenList, TokenOrValue};
use lightningcss::stylesheet::ParserOptions;
use lightningcss::traits::ParseWithOptions;

use crate::parser::css::Property;
use crate::searcher::SearchCondition;

pub struct VariableUsages;

impl SearchCondition for VariableUsages {
    fn matches(&self, prop: &Property) -> bool {
        let Ok(token_list) =
            TokenList::parse_string_with_options(prop.value.raw, ParserOptions::default())
        else {
            return false;
        };
        has_var_reference(&token_list)
    }
}

fn has_var_reference(token_list: &TokenList<'_>) -> bool {
    for token in &token_list.0 {
        match token {
            TokenOrValue::Var(_) | TokenOrValue::DashedIdent(_) => return true,
            TokenOrValue::Function(func) => {
                if has_var_reference(&func.arguments) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::css::{Property, PropertyIdent, PropertyValue};
    use std::path::Path;

    fn prop_with_value(value: &str) -> Property<'_> {
        Property {
            file_path: Path::new("test.css"),
            name: PropertyIdent {
                raw: "color",
                offset: 0,
                line: 0,
                column: 0,
            },
            value: PropertyValue {
                raw: value,
                offset: 0,
                line: 0,
                column: 0,
            },
        }
    }

    #[test]
    fn matches_var_usage() {
        let cond = VariableUsages;
        assert!(cond.matches(&prop_with_value("var(--color)")));
        assert!(cond.matches(&prop_with_value("var(--main-color)")));
        assert!(cond.matches(&prop_with_value("1px solid var(--border-color)")));
    }

    #[test]
    fn matches_bare_dashed_ident() {
        let cond = VariableUsages;
        assert!(cond.matches(&prop_with_value("--my-animation")));
    }

    #[test]
    fn matches_nested_in_function() {
        let cond = VariableUsages;
        assert!(cond.matches(&prop_with_value("calc(var(--size) * 2)")));
    }

    #[test]
    fn rejects_no_var() {
        let cond = VariableUsages;
        assert!(!cond.matches(&prop_with_value("red")));
        assert!(!cond.matches(&prop_with_value("16px")));
    }
}
