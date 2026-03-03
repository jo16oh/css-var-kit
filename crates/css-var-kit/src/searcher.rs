use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;

use crate::parser::css::{ParseResult, Property};

pub trait SearchCondition: 'static {
    fn judge(&self, prop: &Property) -> bool;
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

        for prop in self.parse_result.properties.iter() {
            for (type_id, cond) in self.conditions.iter() {
                if cond.judge(prop) {
                    results.entry(*type_id).or_default().push(prop);
                }
            }
        }

        SearchResult(results)
    }
}

pub struct SearchResult<'a>(HashMap<TypeId, Vec<&'a Property<'a>>>);

impl<'a> SearchResult<'a> {
    pub fn get_result_for<T: SearchCondition>(&'a self, cond: T) -> Option<SearchResultFor<'a, T>> {
        self.0
            .get(&cond.type_id())
            .map(|r| SearchResultFor(r, PhantomData::<T>))
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

    #[test]
    fn test() {
        struct All;

        impl SearchCondition for All {
            fn judge(&self, _props: &Property) -> bool {
                true
            }
        }

        let css = ".a { color: red; }";
        let parse_result = parser::css::parse(css);
        let searcher = SearcherBuilder::new(&parse_result)
            .add_condition(All)
            .build();

        let search_result = searcher.search();
        let result_for_all = search_result.get_result_for(All).unwrap();

        assert_eq!(result_for_all.0[0].name.raw, "color");
        assert_eq!(result_for_all.0[0].value.raw, "red");
    }
}
