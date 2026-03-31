use crate::parser::css::Property as CssProperty;
use crate::searcher::SearchCondition;

pub struct VariableUsages;

impl SearchCondition for VariableUsages {
    fn matches(&self, prop: &CssProperty) -> bool {
        prop.value.raw.contains("var(") || prop.value.raw.contains("--")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use std::path::Path;

    fn matches_value(value: &str) -> bool {
        let css = format!(".a {{ color: {}; }}", value);
        let result = parser::css::parse(&css, Path::new("test.css"));
        let cond = VariableUsages;
        cond.matches(&result.properties[0])
    }

    #[test]
    fn matches_var_usage() {
        assert!(matches_value("var(--color)"));
        assert!(matches_value("var(--main-color)"));
        assert!(matches_value("1px solid var(--border-color)"));
    }

    #[test]
    fn matches_bare_dashed_ident() {
        assert!(matches_value("--my-animation"));
    }

    #[test]
    fn matches_nested_in_function() {
        assert!(matches_value("calc(var(--size) * 2)"));
    }

    #[test]
    fn rejects_no_var() {
        assert!(!matches_value("red"));
        assert!(!matches_value("16px"));
    }
}
