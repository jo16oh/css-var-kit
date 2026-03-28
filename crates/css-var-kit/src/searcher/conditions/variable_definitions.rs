use std::collections::HashMap;

use lightningcss::properties::custom::TokenList;

use crate::parser::css::Property;
use crate::searcher::{PropMapFor, SearchCondition};

pub type VarsMap<'src> = HashMap<&'src str, TokenList<'src>>;

pub struct VariableDefinitions;

impl SearchCondition for VariableDefinitions {
    fn matches(&self, prop: &Property) -> bool {
        prop.name.raw.starts_with("--")
    }
}

impl<'src> PropMapFor<'src, '_, VariableDefinitions> {
    pub fn vars_map(&self) -> VarsMap<'src> {
        self.values()
            .filter_map(|props| {
                let prop = props.last()?;
                let token_list = prop.value.token_list.as_ref()?.clone();
                Some((prop.name.raw, token_list))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use std::path::Path;

    use lightningcss::properties::PropertyId;

    use crate::parser::css::{ParseResult, Property, PropertyIdent, PropertyValue};
    use crate::searcher::SearcherBuilder;

    fn test_parse(css: &str) -> ParseResult<'_> {
        parser::css::parse(css, Path::new("test.css"))
    }

    fn prop(name: &str) -> Property<'_> {
        Property {
            file_path: Path::new("test.css"),
            source: "",
            name: PropertyIdent {
                raw: name,
                unescaped: name.into(),
                property_id: PropertyId::from(name),
                offset: 0,
                line: 0,
                column: 0,
            },
            value: PropertyValue {
                raw: "",
                token_list: None,
                offset: 0,
                line: 0,
                column: 0,
            },
            ignore_comments: vec![],
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
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(&parse_results)
            .add_condition(VariableDefinitions)
            .build();
        let search_result = searcher.search();
        let map = search_result.get_prop_map_for::<VariableDefinitions>();

        let props = map.get(&PropertyId::from("--color")).unwrap();
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].value.raw, "red");

        let props = map.get(&PropertyId::from("--size")).unwrap();
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].value.raw, "16px");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let css = ":root { --color: red; }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(&parse_results)
            .add_condition(VariableDefinitions)
            .build();
        let search_result = searcher.search();
        let map = search_result.get_prop_map_for::<VariableDefinitions>();

        assert!(map.get(&PropertyId::from("--missing")).is_none());
    }

    #[test]
    fn contains_key_returns_correct_bool() {
        let css = ":root { --color: red; }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(&parse_results)
            .add_condition(VariableDefinitions)
            .build();
        let search_result = searcher.search();
        let map = search_result.get_prop_map_for::<VariableDefinitions>();

        assert!(map.contains_key(&PropertyId::from("--color")));
        assert!(!map.contains_key(&PropertyId::from("--missing")));
    }

    #[test]
    fn duplicate_definitions_grouped() {
        let css = ":root { --color: red; } .dark { --color: blue; }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(&parse_results)
            .add_condition(VariableDefinitions)
            .build();
        let search_result = searcher.search();
        let map = search_result.get_prop_map_for::<VariableDefinitions>();

        let props = map.get(&PropertyId::from("--color")).unwrap();
        assert_eq!(props.len(), 2);
        assert_eq!(props[0].value.raw, "red");
        assert_eq!(props[1].value.raw, "blue");
    }
}
