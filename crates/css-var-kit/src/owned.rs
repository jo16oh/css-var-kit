#![allow(dead_code)]

use lightningcss::properties::PropertyId;
use lightningcss::properties::custom::TokenList;
use lightningcss::stylesheet::ParserOptions;
use lightningcss::traits::ParseWithOptions;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;
use yoke::{Yoke, Yokeable};

#[derive(Debug, Error)]
#[error("failed to parse token list: {0}")]
pub struct TokenListParseError(String);

#[derive(Yokeable, Debug, Clone)]
struct YokeableStr<'a>(&'a str);

#[derive(Debug, Clone)]
pub struct OwnedStr(Yoke<YokeableStr<'static>, Rc<str>>);

impl OwnedStr {
    pub fn backing_rc(&self) -> &Rc<str> {
        self.0.backing_cart()
    }

    pub fn map<F>(&self, f: F) -> Self
    where
        F: for<'a> FnOnce(&'a str) -> &'a str,
    {
        Self(self.0.clone().map_project(|y, _c| YokeableStr(f(y.0))))
    }

    pub fn slice(&self, range: std::ops::Range<usize>) -> Self {
        self.map(|s| &s[range])
    }
}

impl From<Rc<str>> for OwnedStr {
    fn from(rc: Rc<str>) -> Self {
        Self(Yoke::attach_to_cart(rc, |s| YokeableStr(s)))
    }
}

impl From<&str> for OwnedStr {
    fn from(s: &str) -> Self {
        Self::from(Rc::<str>::from(s.to_string()))
    }
}

impl From<String> for OwnedStr {
    fn from(s: String) -> Self {
        Self::from(Rc::<str>::from(s))
    }
}

impl From<&String> for OwnedStr {
    fn from(s: &String) -> Self {
        Self::from(Rc::<str>::from(s.as_str()))
    }
}

impl PartialEq for OwnedStr {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Eq for OwnedStr {}

impl Deref for OwnedStr {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.0.get().0
    }
}

impl std::fmt::Display for OwnedStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self)
    }
}

use std::ops::Range;

#[derive(Yokeable, Debug, Clone)]
struct YokeablePropId<'a>(PropertyId<'a>);

#[derive(Debug, Clone)]
pub struct OwnedPropId {
    yoke: Yoke<YokeablePropId<'static>, Rc<str>>,
    range: Range<usize>,
}

impl PartialEq for OwnedPropId {
    fn eq(&self, other: &Self) -> bool {
        self.inner() == other.inner()
    }
}

impl Eq for OwnedPropId {}

impl Hash for OwnedPropId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner().hash(state);
    }
}

impl From<&OwnedStr> for OwnedPropId {
    fn from(ident: &OwnedStr) -> Self {
        let rc = ident.backing_rc().clone();
        let base = rc.as_ptr() as usize;
        let start = (**ident).as_ptr() as usize - base;
        let end = start + ident.len();
        let range = start..end;

        Self {
            yoke: Yoke::attach_to_cart(rc, move |full| {
                YokeablePropId(PropertyId::from(&full[start..end]))
            }),
            range,
        }
    }
}

impl From<String> for OwnedPropId {
    fn from(value: String) -> Self {
        let owned_str = OwnedStr::from(value.to_string());
        OwnedPropId::from(&owned_str)
    }
}

impl OwnedPropId {
    pub fn inner(&'_ self) -> &'_ PropertyId<'_> {
        &self.yoke.get().0
    }

    pub fn as_str(&self) -> &str {
        &self.yoke.backing_cart()[self.range.clone()]
    }
}

#[derive(Yokeable, Debug, Clone)]
struct YokeableTokenList<'a>(TokenList<'a>);

#[derive(Debug, Clone)]
pub struct OwnedTokenList(Yoke<YokeableTokenList<'static>, Rc<str>>);

impl PartialEq for OwnedTokenList {
    fn eq(&self, other: &Self) -> bool {
        self.inner() == other.inner()
    }
}

impl Eq for OwnedTokenList {}

impl Hash for OwnedTokenList {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner().hash(state);
    }
}

impl Default for OwnedTokenList {
    fn default() -> Self {
        Self(Yoke::attach_to_cart(Rc::from(""), |_| {
            YokeableTokenList(TokenList(vec![]))
        }))
    }
}

impl OwnedTokenList {
    pub fn parse(str: &OwnedStr) -> Result<Self, TokenListParseError> {
        let cart = str.backing_rc().clone();
        Yoke::try_attach_to_cart(cart, |s| {
            TokenList::parse_string_with_options(s, ParserOptions::default())
                .map(YokeableTokenList)
                .map_err(|e| TokenListParseError(e.to_string()))
        })
        .map(Self)
    }

    pub fn inner(&'_ self) -> &'_ TokenList<'_> {
        &self.0.get().0
    }
}
