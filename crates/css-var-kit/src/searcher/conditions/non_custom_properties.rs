use crate::parser::css::Property;
use crate::searcher::SearchCondition;

pub struct NonCustomProperties;

impl SearchCondition for NonCustomProperties {
    fn matches(&self, prop: &Property) -> bool {
        !prop.name.raw.starts_with("--")
    }
}
