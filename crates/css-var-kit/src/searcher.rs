use std::any::TypeId;
use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::ops::Deref;
use std::path::Path;
use std::rc::Rc;

use crate::{
    owned_types::OwnedPropId,
    parser::{ParseResult, Property},
    searcher::conditions::variable_definitions::VariableDefinitions,
};

pub mod conditions;

pub trait SearchCondition: 'static {
    fn matches(&self, prop: &Property) -> bool;
}

pub struct Searcher {
    conditions: HashMap<TypeId, Box<dyn SearchCondition>>,
    per_file: HashMap<Rc<Path>, HashMap<TypeId, Vec<Property>>>,
}

impl Searcher {
    pub fn new() -> Self {
        Self {
            conditions: HashMap::new(),
            per_file: HashMap::new(),
        }
    }

    pub fn add_condition<T: SearchCondition>(mut self, cond: T) -> Self {
        self.conditions.insert(TypeId::of::<T>(), Box::new(cond));
        self
    }

    pub fn update_file(&mut self, path: &Rc<Path>, parse_results: &[ParseResult]) {
        let mut file_results: HashMap<TypeId, Vec<Property>> = self
            .conditions
            .keys()
            .map(|&type_id| (type_id, Vec::new()))
            .collect();

        for parse_result in parse_results {
            for prop in &parse_result.properties {
                for (type_id, cond) in &self.conditions {
                    if cond.matches(prop) {
                        file_results.get_mut(type_id).unwrap().push(prop.clone());
                    }
                }
            }
        }

        self.per_file.insert(path.clone(), file_results);
    }

    pub fn remove_file(&mut self, path: &Path) {
        self.per_file.remove(path);
    }

    pub fn search(&self) -> SearchResult {
        let mut results: HashMap<TypeId, SearchConditionResult> = self
            .conditions
            .keys()
            .map(|&type_id| {
                let props: Vec<Property> = self
                    .per_file
                    .values()
                    .filter_map(|file_results| file_results.get(&type_id))
                    .flatten()
                    .cloned()
                    .collect();
                (
                    type_id,
                    SearchConditionResult {
                        props,
                        prop_map: OnceCell::new(),
                    },
                )
            })
            .collect();

        // Ensure all condition types are present even if per_file is empty
        for type_id in self.conditions.keys() {
            results
                .entry(*type_id)
                .or_insert_with(|| SearchConditionResult {
                    props: Vec::new(),
                    prop_map: OnceCell::new(),
                });
        }

        SearchResult { results }
    }

    pub fn search_for_files(&self, target_files: &HashSet<Rc<Path>>) -> SearchResult {
        let def_type_id = TypeId::of::<VariableDefinitions>();
        let results = self
            .conditions
            .keys()
            .map(|&type_id| {
                let props: Vec<Property> = self
                    .per_file
                    .iter()
                    .filter(|(path, _)| {
                        type_id == def_type_id || target_files.contains(path.as_ref())
                    })
                    .filter_map(|(_, file_results)| file_results.get(&type_id))
                    .flatten()
                    .cloned()
                    .collect();
                (
                    type_id,
                    SearchConditionResult {
                        props,
                        prop_map: OnceCell::new(),
                    },
                )
            })
            .collect();
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
            .expect("condition not registered in Searcher");
        SearchResultFor(&entry.props, PhantomData::<T>)
    }

    pub fn get_prop_map_for<T: SearchCondition>(&self) -> PropMapFor<'_, T> {
        let entry = self
            .results
            .get(&TypeId::of::<T>())
            .expect("condition not registered in Searcher");
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
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use crate::{owned_types::OwnedStr, parser};

    use super::*;

    fn search_css(css: &str, searcher: Searcher) -> SearchResult {
        let parse_result = parser::css::parse(
            &OwnedStr::from(css),
            &Rc::from(PathBuf::from("test.css".to_string())),
        );
        let file_path: Rc<Path> = Rc::from(parse_result.file_path.as_ref());
        let mut searcher = searcher;
        searcher.update_file(&file_path, std::slice::from_ref(&parse_result));
        searcher.search()
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
        let result = search_css(
            ".a { color: red; font-size: 16px; margin: 0; }",
            Searcher::new().add_condition(All),
        );
        let props = result.get_result_for(All);

        assert_eq!(props.len(), 3);
        assert_eq!(props[0].ident.raw.as_str(), "color");
        assert_eq!(props[1].ident.raw.as_str(), "font-size");
        assert_eq!(props[2].ident.raw.as_str(), "margin");
    }

    #[test]
    fn match_none_returns_empty() {
        let result = search_css(".a { color: red; }", Searcher::new().add_condition(None));
        let props = result.get_result_for(None);
        assert!(props.is_empty());
    }

    #[test]
    fn filter_by_name() {
        let result = search_css(
            ".a { color: red; font-size: 16px; color: blue; }",
            Searcher::new().add_condition(NameEquals::from("color")),
        );
        let props = result.get_result_for(NameEquals::from("color"));

        assert_eq!(props.len(), 2);
        assert_eq!(props[0].value.raw.as_str(), "red");
        assert_eq!(props[1].value.raw.as_str(), "blue");
    }

    #[test]
    fn filter_by_value() {
        let result = search_css(
            ".a { color: red; background: red; font-size: 16px; }",
            Searcher::new().add_condition(ValueEquals::from("red")),
        );
        let props = result.get_result_for(ValueEquals::from("red"));

        assert_eq!(props.len(), 2);
        assert_eq!(props[0].ident.raw.as_str(), "color");
        assert_eq!(props[1].ident.raw.as_str(), "background");
    }

    #[test]
    fn multiple_conditions() {
        let result = search_css(
            ".a { color: red; font-size: 16px; background: blue; }",
            Searcher::new()
                .add_condition(NameEquals::from("color"))
                .add_condition(ValueEquals::from("16px")),
        );

        let by_name = result.get_result_for(NameEquals::from("color"));
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].value.raw.as_str(), "red");

        let by_value = result.get_result_for(ValueEquals::from("16px"));
        assert_eq!(by_value.len(), 1);
        assert_eq!(by_value[0].ident.raw.as_str(), "font-size");
    }

    #[test]
    #[should_panic(expected = "condition not registered in Searcher")]
    fn unregistered_condition_panics() {
        let result = search_css(".a { color: red; }", Searcher::new());
        result.get_result_for(All);
    }

    #[test]
    fn empty_css() {
        let result = search_css(".a { }", Searcher::new().add_condition(All));
        let props = result.get_result_for(All);
        assert!(props.is_empty());
    }

    #[test]
    fn css_variables() {
        let result = search_css(
            ":root { --primary: #ff0000; --secondary: #00ff00; color: black; }",
            Searcher::new().add_condition(IsVariable),
        );
        let props = result.get_result_for(IsVariable);

        assert_eq!(props.len(), 2);
        assert_eq!(props[0].ident.raw.as_str(), "--primary");
        assert_eq!(props[0].value.raw.as_str(), "#ff0000");
        assert_eq!(props[1].ident.raw.as_str(), "--secondary");
        assert_eq!(props[1].value.raw.as_str(), "#00ff00");
    }

    #[test]
    fn multiple_selectors() {
        let result = search_css(
            ".a { color: red; } .b { color: blue; margin: 0; }",
            Searcher::new().add_condition(NameEquals::from("color")),
        );
        let props = result.get_result_for(NameEquals::from("color"));

        assert_eq!(props.len(), 2);
        assert_eq!(props[0].value.raw.as_str(), "red");
        assert_eq!(props[1].value.raw.as_str(), "blue");
    }

    #[test]
    fn condition_with_no_matches() {
        let result = search_css(
            ".a { color: red; font-size: 16px; }",
            Searcher::new().add_condition(NameEquals::from("background")),
        );
        let props = result.get_result_for(NameEquals::from("background"));
        assert!(props.is_empty());
    }
}
