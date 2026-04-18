use std::any::TypeId;
use std::cell::OnceCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;

use crate::{
    owned::OwnedPropId,
    parser::css::{ParseResult, Property},
};

pub mod conditions;

pub trait SearchCondition: 'static {
    fn matches(&self, prop: &Property) -> bool;
}

pub struct SearcherBuilder {
    parse_results: Vec<ParseResult>,
    conditions: HashMap<TypeId, Box<dyn SearchCondition>>,
}

impl SearcherBuilder {
    pub fn new(parse_results: Vec<ParseResult>) -> Self {
        Self {
            parse_results,
            conditions: HashMap::new(),
        }
    }

    pub fn add_condition<T: SearchCondition>(mut self, cond: T) -> SearcherBuilder {
        self.conditions.insert(TypeId::of::<T>(), Box::new(cond));
        self
    }

    pub fn build(self) -> Searcher {
        Searcher {
            parse_results: self.parse_results,
            conditions: self.conditions,
        }
    }
}

pub struct Searcher {
    parse_results: Vec<ParseResult>,
    conditions: HashMap<TypeId, Box<dyn SearchCondition>>,
}

impl Searcher {
    pub fn search(&self) -> SearchResult {
        let mut results = HashMap::<TypeId, SearchConditionResult>::new();

        for type_id in self.conditions.keys() {
            results.insert(
                *type_id,
                SearchConditionResult {
                    props: Vec::new(),
                    prop_map: OnceCell::new(),
                },
            );
        }

        for parse_result in &self.parse_results {
            for prop in parse_result.properties.iter() {
                for (type_id, cond) in self.conditions.iter() {
                    if cond.matches(prop) {
                        results.get_mut(type_id).unwrap().props.push(prop.clone());
                    }
                }
            }
        }

        SearchResult { results }
    }
}

type PropMapIndices = HashMap<OwnedPropId, Vec<usize>>;

struct SearchConditionResult {
    props: Vec<Property>,
    prop_map: OnceCell<PropMapIndices>,
}

pub struct SearchResult {
    results: HashMap<TypeId, SearchConditionResult>,
}

impl SearchResult {
    pub fn get_result_for<T: SearchCondition>(&self, _cond: T) -> SearchResultFor<'_, T> {
        let entry = self
            .results
            .get(&TypeId::of::<T>())
            .expect("condition not registered in SearcherBuilder");
        SearchResultFor(&entry.props, PhantomData::<T>)
    }

    pub fn get_prop_map_for<T: SearchCondition>(&self) -> PropMapFor<'_, T> {
        let entry = self
            .results
            .get(&TypeId::of::<T>())
            .expect("condition not registered in SearcherBuilder");
        let map = entry.prop_map.get_or_init(|| {
            let mut indices = PropMapIndices::new();
            for (i, prop) in entry.props.iter().enumerate() {
                indices
                    .entry(prop.ident.property_id.clone())
                    .or_default()
                    .push(i);
            }
            indices
        });
        PropMapFor {
            props: &entry.props,
            map,
            _marker: PhantomData::<T>,
        }
    }
}

pub struct SearchResultFor<'result, T: SearchCondition>(&'result [Property], PhantomData<T>);

impl<T: SearchCondition> Deref for SearchResultFor<'_, T> {
    type Target = [Property];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

pub struct PropMapFor<'result, T: SearchCondition> {
    props: &'result [Property],
    map: &'result PropMapIndices,
    _marker: PhantomData<T>,
}

impl<'result, T: SearchCondition> PropMapFor<'result, T> {
    pub fn contains_key(&self, key: &OwnedPropId) -> bool {
        self.map.contains_key(key)
    }

    pub fn get(&self, key: &OwnedPropId) -> Option<Vec<&'result Property>> {
        self.map
            .get(key)
            .map(|indices| indices.iter().map(|&i| &self.props[i]).collect())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'result OwnedPropId, Vec<&'result Property>)> {
        self.map.iter().map(|(k, indices)| {
            (
                k,
                indices.iter().map(|&i| &self.props[i]).collect::<Vec<_>>(),
            )
        })
    }

    pub fn values(&self) -> impl Iterator<Item = Vec<&'result Property>> {
        self.map
            .values()
            .map(|indices| indices.iter().map(|&i| &self.props[i]).collect::<Vec<_>>())
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, rc::Rc};

    use crate::{owned::OwnedStr, parser};

    use super::*;

    fn test_parse(css: &str) -> crate::parser::css::ParseResult {
        parser::css::parse(
            &OwnedStr::from(css),
            &Rc::new(PathBuf::from("test.css".to_string())),
        )
    }

    struct All;
    impl SearchCondition for All {
        fn matches(&self, _prop: &Property) -> bool {
            true
        }
    }

    struct None;
    impl SearchCondition for None {
        fn matches(&self, _prop: &Property) -> bool {
            false
        }
    }

    struct NameEquals(OwnedStr);
    impl SearchCondition for NameEquals {
        fn matches(&self, prop: &Property) -> bool {
            prop.ident.raw == self.0
        }
    }

    impl From<&str> for NameEquals {
        fn from(value: &str) -> Self {
            Self(OwnedStr::from(value))
        }
    }

    struct ValueEquals(OwnedStr);
    impl SearchCondition for ValueEquals {
        fn matches(&self, prop: &Property) -> bool {
            prop.value.raw == self.0
        }
    }

    impl From<&str> for ValueEquals {
        fn from(value: &str) -> Self {
            Self(OwnedStr::from(value))
        }
    }

    struct IsVariable;
    impl SearchCondition for IsVariable {
        fn matches(&self, prop: &Property) -> bool {
            prop.ident.raw.starts_with("--")
        }
    }

    #[test]
    fn match_all_properties() {
        let css = ".a { color: red; font-size: 16px; margin: 0; }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results.to_vec())
            .add_condition(All)
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(All);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].ident.raw.as_str(), "color");
        assert_eq!(result[1].ident.raw.as_str(), "font-size");
        assert_eq!(result[2].ident.raw.as_str(), "margin");
    }

    #[test]
    fn match_none_returns_empty() {
        let css = ".a { color: red; }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results.to_vec())
            .add_condition(None)
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(None);

        assert!(result.is_empty());
    }

    #[test]
    fn filter_by_name() {
        let css = ".a { color: red; font-size: 16px; color: blue; }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results.to_vec())
            .add_condition(NameEquals::from("color"))
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(NameEquals::from("color"));

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].value.raw.as_str(), "red");
        assert_eq!(result[1].value.raw.as_str(), "blue");
    }

    #[test]
    fn filter_by_value() {
        let css = ".a { color: red; background: red; font-size: 16px; }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results.to_vec())
            .add_condition(ValueEquals::from("red"))
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(ValueEquals::from("red"));

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].ident.raw.as_str(), "color");
        assert_eq!(result[1].ident.raw.as_str(), "background");
    }

    #[test]
    fn multiple_conditions() {
        let css = ".a { color: red; font-size: 16px; background: blue; }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results.to_vec())
            .add_condition(NameEquals::from("color"))
            .add_condition(ValueEquals::from("16px"))
            .build();

        let search_result = searcher.search();

        let by_name = search_result.get_result_for(NameEquals::from("color"));
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].value.raw.as_str(), "red");

        let by_value = search_result.get_result_for(ValueEquals::from("16px"));
        assert_eq!(by_value.len(), 1);
        assert_eq!(by_value[0].ident.raw.as_str(), "font-size");
    }

    #[test]
    #[should_panic(expected = "condition not registered in SearcherBuilder")]
    fn unregistered_condition_panics() {
        let css = ".a { color: red; }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results.to_vec()).build();

        let search_result = searcher.search();
        search_result.get_result_for(All);
    }

    #[test]
    fn empty_css() {
        let css = ".a { }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results.to_vec())
            .add_condition(All)
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(All);

        assert!(result.is_empty());
    }

    #[test]
    fn css_variables() {
        let css = ":root { --primary: #ff0000; --secondary: #00ff00; color: black; }";
        let parse_results = [test_parse(css)];

        let searcher = SearcherBuilder::new(parse_results.to_vec())
            .add_condition(IsVariable)
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(IsVariable);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].ident.raw.as_str(), "--primary");
        assert_eq!(result[0].value.raw.as_str(), "#ff0000");
        assert_eq!(result[1].ident.raw.as_str(), "--secondary");
        assert_eq!(result[1].value.raw.as_str(), "#00ff00");
    }

    #[test]
    fn multiple_selectors() {
        let css = ".a { color: red; } .b { color: blue; margin: 0; }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results.to_vec())
            .add_condition(NameEquals::from("color"))
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(NameEquals::from("color"));

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].value.raw.as_str(), "red");
        assert_eq!(result[1].value.raw.as_str(), "blue");
    }

    #[test]
    fn condition_with_no_matches() {
        let css = ".a { color: red; font-size: 16px; }";
        let parse_results = [test_parse(css)];
        let searcher = SearcherBuilder::new(parse_results.to_vec())
            .add_condition(NameEquals::from("background"))
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(NameEquals::from("background"));

        assert!(result.is_empty());
    }
}
