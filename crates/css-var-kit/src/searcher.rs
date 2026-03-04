use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;

use crate::parser::css::{ParseResult, Property};

pub mod conditions;

pub trait SearchCondition: 'static {
    fn matches(&self, prop: &Property) -> bool;
}

pub struct SearcherBuilder<'a> {
    parse_result: &'a ParseResult<'a>,
    conditions: HashMap<TypeId, Box<dyn SearchCondition>>,
}

impl<'a> SearcherBuilder<'a> {
    pub fn new(parse_result: &'a ParseResult) -> Self {
        Self {
            parse_result,
            conditions: HashMap::new(),
        }
    }

    pub fn add_condition<T: SearchCondition>(mut self, cond: T) -> SearcherBuilder<'a> {
        self.conditions.insert(TypeId::of::<T>(), Box::new(cond));
        self
    }

    pub fn build(self) -> Searcher<'a> {
        Searcher {
            parse_result: self.parse_result,
            conditions: self.conditions,
        }
    }
}

pub struct Searcher<'a> {
    parse_result: &'a ParseResult<'a>,
    conditions: HashMap<TypeId, Box<dyn SearchCondition>>,
}

impl<'a> Searcher<'a> {
    pub fn search(&'_ self) -> SearchResult<'_> {
        let mut results = HashMap::<TypeId, Vec<&'a Property<'a>>>::new();

        for type_id in self.conditions.keys() {
            results.insert(*type_id, Vec::new());
        }

        for prop in self.parse_result.properties.iter() {
            for (type_id, cond) in self.conditions.iter() {
                if cond.matches(prop) {
                    results.get_mut(type_id).unwrap().push(prop);
                }
            }
        }

        SearchResult(results)
    }
}

pub struct SearchResult<'a>(HashMap<TypeId, Vec<&'a Property<'a>>>);

impl<'a> SearchResult<'a> {
    pub fn get_result_for<T: SearchCondition>(&'a self, cond: T) -> SearchResultFor<'a, T> {
        let props = self
            .0
            .get(&cond.type_id())
            .expect("condition not registered in SearcherBuilder");
        SearchResultFor(props, PhantomData::<T>)
    }
}

pub struct SearchResultFor<'a, T: SearchCondition>(&'a [&'a Property<'a>], PhantomData<T>);

impl<'a, T: SearchCondition> Deref for SearchResultFor<'a, T> {
    type Target = [&'a Property<'a>];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::parser;

    use super::*;

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

    struct NameEquals(&'static str);
    impl SearchCondition for NameEquals {
        fn matches(&self, prop: &Property) -> bool {
            prop.name.raw == self.0
        }
    }

    struct ValueEquals(&'static str);
    impl SearchCondition for ValueEquals {
        fn matches(&self, prop: &Property) -> bool {
            prop.value.raw == self.0
        }
    }

    struct IsVariable;
    impl SearchCondition for IsVariable {
        fn matches(&self, prop: &Property) -> bool {
            prop.name.raw.starts_with("--")
        }
    }

    #[test]
    fn match_all_properties() {
        let css = ".a { color: red; font-size: 16px; margin: 0; }";
        let parse_result = parser::css::parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(All)
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(All);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].name.raw, "color");
        assert_eq!(result[1].name.raw, "font-size");
        assert_eq!(result[2].name.raw, "margin");
    }

    #[test]
    fn match_none_returns_empty() {
        let css = ".a { color: red; }";
        let parse_result = parser::css::parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(None)
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(None);

        assert!(result.is_empty());
    }

    #[test]
    fn filter_by_name() {
        let css = ".a { color: red; font-size: 16px; color: blue; }";
        let parse_result = parser::css::parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(NameEquals("color"))
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(NameEquals("color"));

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].value.raw, "red");
        assert_eq!(result[1].value.raw, "blue");
    }

    #[test]
    fn filter_by_value() {
        let css = ".a { color: red; background: red; font-size: 16px; }";
        let parse_result = parser::css::parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(ValueEquals("red"))
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(ValueEquals("red"));

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name.raw, "color");
        assert_eq!(result[1].name.raw, "background");
    }

    #[test]
    fn multiple_conditions() {
        let css = ".a { color: red; font-size: 16px; background: blue; }";
        let parse_result = parser::css::parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(NameEquals("color"))
            .add_condition(ValueEquals("16px"))
            .build();

        let search_result = searcher.search();

        let by_name = search_result.get_result_for(NameEquals("color"));
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].value.raw, "red");

        let by_value = search_result.get_result_for(ValueEquals("16px"));
        assert_eq!(by_value.len(), 1);
        assert_eq!(by_value[0].name.raw, "font-size");
    }

    #[test]
    #[should_panic(expected = "condition not registered in SearcherBuilder")]
    fn unregistered_condition_panics() {
        let css = ".a { color: red; }";
        let parse_result = parser::css::parse(css);
        let searcher = SearcherBuilder::new(&parse_result).build();

        let search_result = searcher.search();
        search_result.get_result_for(All);
    }

    #[test]
    fn empty_css() {
        let css = ".a { }";
        let parse_result = parser::css::parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(All)
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(All);

        assert!(result.is_empty());
    }

    #[test]
    fn css_variables() {
        let css = ":root { --primary: #ff0000; --secondary: #00ff00; color: black; }";
        let parse_result = parser::css::parse(css);

        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(IsVariable)
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(IsVariable);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name.raw, "--primary");
        assert_eq!(result[0].value.raw, "#ff0000");
        assert_eq!(result[1].name.raw, "--secondary");
        assert_eq!(result[1].value.raw, "#00ff00");
    }

    #[test]
    fn multiple_selectors() {
        let css = ".a { color: red; } .b { color: blue; margin: 0; }";
        let parse_result = parser::css::parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(NameEquals("color"))
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(NameEquals("color"));

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].value.raw, "red");
        assert_eq!(result[1].value.raw, "blue");
    }

    #[test]
    fn condition_with_no_matches() {
        let css = ".a { color: red; font-size: 16px; }";
        let parse_result = parser::css::parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(NameEquals("background"))
            .build();

        let search_result = searcher.search();
        let result = search_result.get_result_for(NameEquals("background"));

        assert!(result.is_empty());
    }
}
