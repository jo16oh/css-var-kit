use std::collections::HashMap;

use crate::parser::css::Property;
use crate::searcher::SearchCondition;
use crate::searcher::SearchResultFor;

pub struct VariableDefinitions;

impl SearchCondition for VariableDefinitions {
    fn matches(&self, prop: &Property) -> bool {
        prop.name.raw.starts_with("--")
    }
}

pub struct VariableDefinitionMap<'a> {
    map: HashMap<&'a str, Vec<&'a Property<'a>>>,
}

impl<'a> From<&SearchResultFor<'a, VariableDefinitions>> for VariableDefinitionMap<'a> {
    fn from(result: &SearchResultFor<'a, VariableDefinitions>) -> Self {
        let mut map = HashMap::<&'a str, Vec<&'a Property<'a>>>::new();
        for prop in result.iter() {
            map.entry(prop.name.raw).or_default().push(prop);
        }
        Self { map }
    }
}

impl<'a> VariableDefinitionMap<'a> {
    pub fn get(&self, name: &str) -> Option<&[&'a Property<'a>]> {
        self.map.get(name).map(|v| v.as_slice())
    }

    pub fn has(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use std::path::Path;

    use crate::parser::css::{ParseResult, Property, PropertyIdent, PropertyValue};
    use crate::searcher::SearcherBuilder;

    fn test_parse(css: &str) -> ParseResult<'_> {
        parser::css::parse(css, Path::new("test.css"))
    }

    fn prop(name: &str) -> Property<'_> {
        Property {
            file_path: Path::new("test.css"),
            name: PropertyIdent {
                raw: name,
                offset: 0,
                line: 0,
                column: 0,
            },
            value: PropertyValue {
                raw: "",
                offset: 0,
                line: 0,
                column: 0,
            },
        }
    }

    #[test]
    fn matches_css_variable() {
        let cond = VariableDefinitions;
        assert!(cond.matches(&prop("--color")));
        assert!(cond.matches(&prop("--main-color")));
        assert!(cond.matches(&prop("--")));
    }

    #[test]
    fn rejects_regular_property() {
        let cond = VariableDefinitions;
        assert!(!cond.matches(&prop("color")));
        assert!(!cond.matches(&prop("font-size")));
        assert!(!cond.matches(&prop("background-color")));
    }

    #[test]
    fn rejects_single_hyphen() {
        let cond = VariableDefinitions;
        assert!(!cond.matches(&prop("-webkit-transform")));
        assert!(!cond.matches(&prop("-moz-appearance")));
    }

    #[test]
    fn get_by_name() {
        let css = ":root { --color: red; --size: 16px; }";
        let parse_result = test_parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(VariableDefinitions)
            .build();
        let search_result = searcher.search();
        let result = search_result.get_result_for(VariableDefinitions);
        let map = VariableDefinitionMap::from(&result);

        let props = map.get("--color").unwrap();
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].value.raw, "red");

        let props = map.get("--size").unwrap();
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].value.raw, "16px");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let css = ":root { --color: red; }";
        let parse_result = test_parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(VariableDefinitions)
            .build();
        let search_result = searcher.search();
        let result = search_result.get_result_for(VariableDefinitions);
        let map = VariableDefinitionMap::from(&result);

        assert!(map.get("--missing").is_none());
    }

    #[test]
    fn has_returns_correct_bool() {
        let css = ":root { --color: red; }";
        let parse_result = test_parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(VariableDefinitions)
            .build();
        let search_result = searcher.search();
        let result = search_result.get_result_for(VariableDefinitions);
        let map = VariableDefinitionMap::from(&result);

        assert!(map.has("--color"));
        assert!(!map.has("--missing"));
    }

    #[test]
    fn duplicate_definitions_grouped() {
        let css = ":root { --color: red; } .dark { --color: blue; }";
        let parse_result = test_parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(VariableDefinitions)
            .build();
        let search_result = searcher.search();
        let result = search_result.get_result_for(VariableDefinitions);
        let map = VariableDefinitionMap::from(&result);

        let props = map.get("--color").unwrap();
        assert_eq!(props.len(), 2);
        assert_eq!(props[0].value.raw, "red");
        assert_eq!(props[1].value.raw, "blue");
    }
}
