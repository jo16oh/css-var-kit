use std::collections::HashMap;

use lightningcss::properties::custom::TokenList;

use crate::config::LookupFilesMatcher;
use crate::parser::css::Property as CssProperty;
use crate::searcher::{PropMapFor, SearchCondition};

pub type VarsMap<'a> = HashMap<&'a str, TokenList<'a>>;

#[derive(Default)]
pub struct VariableDefinitions {
    definition_files: LookupFilesMatcher,
    include: LookupFilesMatcher,
}

impl VariableDefinitions {
    pub fn new(definition_files: LookupFilesMatcher, include: LookupFilesMatcher) -> Self {
        Self {
            definition_files,
            include,
        }
    }
}

impl SearchCondition for VariableDefinitions {
    fn matches(&self, prop: &CssProperty) -> bool {
        prop.ident.raw.starts_with("--")
            && (self.definition_files.matches(prop.file_path.as_ref())
                || self.include.matches(prop.file_path.as_ref()))
    }
}

impl PropMapFor<'_, VariableDefinitions> {
    pub fn vars_map(&self) -> VarsMap<'_> {
        self.values()
            .filter_map(|props| {
                let prop = props.last()?;
                let token_list = prop.token_list().inner().clone();
                if token_list.0.is_empty() {
                    return None;
                }
                Some((&*prop.ident.raw, token_list))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned::{OwnedPropId, OwnedStr};
    use crate::parser;
    use std::path::PathBuf;
    use std::rc::Rc;

    use crate::parser::css::ParseResult;
    use crate::searcher::SearcherBuilder;

    fn test_parse(css: &str) -> ParseResult {
        parser::css::parse(&OwnedStr::from(css), &Rc::new(PathBuf::from("test.css")))
    }

    #[test]
    fn matches_css_variable() {
        let parse_result = test_parse(":root { --color: red; --main-color: blue; --: green; }");
        let cond = VariableDefinitions::default();
        for prop in &parse_result.properties {
            if prop.ident.raw.starts_with("--") {
                assert!(cond.matches(prop));
            }
        }
    }

    #[test]
    fn rejects_regular_property() {
        let parse_result =
            test_parse(".a { color: red; font-size: 16px; background-color: blue; }");
        let cond = VariableDefinitions::default();
        for prop in &parse_result.properties {
            assert!(!cond.matches(prop));
        }
    }

    #[test]
    fn rejects_single_hyphen() {
        let parse_result = test_parse(".a { -webkit-transform: none; }");
        let cond = VariableDefinitions::default();
        for prop in &parse_result.properties {
            assert!(!cond.matches(prop));
        }
    }

    #[test]
    fn get_by_name() {
        let css = ":root { --color: red; --size: 16px; }";
        let parse_results = vec![test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results)
            .add_condition(VariableDefinitions::default())
            .build();
        let search_result = searcher.search();
        let map = search_result.get_prop_map_for::<VariableDefinitions>();

        let color_id = OwnedPropId::from("--color".to_string());
        let props = map.get(&color_id).unwrap();
        assert_eq!(props.len(), 1);
        assert_eq!(&*props[0].value.raw, "red");

        let size_id = OwnedPropId::from("--size".to_string());
        let props = map.get(&size_id).unwrap();
        assert_eq!(props.len(), 1);
        assert_eq!(&*props[0].value.raw, "16px");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let css = ":root { --color: red; }";
        let parse_results = vec![test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results)
            .add_condition(VariableDefinitions::default())
            .build();
        let search_result = searcher.search();
        let map = search_result.get_prop_map_for::<VariableDefinitions>();

        let missing_id = OwnedPropId::from("--missing".to_string());
        assert!(map.get(&missing_id).is_none());
    }

    #[test]
    fn contains_key_returns_correct_bool() {
        let css = ":root { --color: red; }";
        let parse_results = vec![test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results)
            .add_condition(VariableDefinitions::default())
            .build();
        let search_result = searcher.search();
        let map = search_result.get_prop_map_for::<VariableDefinitions>();

        let color_id = OwnedPropId::from("--color".to_string());
        assert!(map.contains_key(&color_id));
        let missing_id = OwnedPropId::from("--missing".to_string());
        assert!(!map.contains_key(&missing_id));
    }

    #[test]
    fn duplicate_definitions_grouped() {
        let css = ":root { --color: red; } .dark { --color: blue; }";
        let parse_results = vec![test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results)
            .add_condition(VariableDefinitions::default())
            .build();
        let search_result = searcher.search();
        let map = search_result.get_prop_map_for::<VariableDefinitions>();

        let color_id = OwnedPropId::from("--color".to_string());
        let props = map.get(&color_id).unwrap();
        assert_eq!(props.len(), 2);
        assert_eq!(&*props[0].value.raw, "red");
        assert_eq!(&*props[1].value.raw, "blue");
    }
}
