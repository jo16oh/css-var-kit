use crate::parser::css::Property as CssProperty;
use crate::searcher::SearchCondition;

pub struct VariableUsages;

impl SearchCondition for VariableUsages {
    fn matches(&self, prop: &CssProperty) -> bool {
        prop.value.raw.contains("var(") || has_dashed_ident(prop.value.raw)
    }
}

fn has_dashed_ident(value: &str) -> bool {
    let bytes = value.as_bytes();
    (0..bytes.len().saturating_sub(2)).any(|i| {
        bytes[i] == b'-'
            && bytes[i + 1] == b'-'
            && (bytes[i + 2].is_ascii_alphanumeric()
                || bytes[i + 2] == b'_'
                || bytes[i + 2] == b'-')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned::OwnedStr;
    use crate::parser;
    use std::path::PathBuf;
    use std::rc::Rc;

    fn matches_value(value: &str) -> bool {
        let css = format!(".a {{ color: {}; }}", value);
        let result = parser::css::parse(OwnedStr::from(css), Rc::new(PathBuf::from("test.css")));
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

    #[test]
    fn rejects_bare_double_dash() {
        assert!(!matches_value("--"));
    }
}
