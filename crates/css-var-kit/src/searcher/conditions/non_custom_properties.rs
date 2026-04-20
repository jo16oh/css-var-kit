use crate::parser::Property;
use crate::searcher::SearchCondition;

pub struct NonCustomProperties;

impl SearchCondition for NonCustomProperties {
    fn matches(&self, prop: &Property) -> bool {
        !prop.ident.raw.starts_with("--")
    }
}
