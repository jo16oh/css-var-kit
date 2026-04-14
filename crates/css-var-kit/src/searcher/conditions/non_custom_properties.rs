use crate::parser::css::Property as CssProperty;
use crate::searcher::SearchCondition;

pub struct NonCustomProperties;

impl SearchCondition for NonCustomProperties {
    fn matches(&self, prop: &CssProperty) -> bool {
        !prop.ident.raw.starts_with("--")
    }
}
