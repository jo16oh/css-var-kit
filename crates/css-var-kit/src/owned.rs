#![allow(dead_code)]

use lightningcss::properties::PropertyId;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;
use yoke::{Yoke, Yokeable};

#[derive(Yokeable)]
struct YokeableStr<'a>(&'a str);

pub struct OwnedStr(Yoke<YokeableStr<'static>, Rc<str>>);

impl OwnedStr {
    pub fn sub_slice(rc: Rc<str>, start: usize, end: usize) -> Self {
        Self(Yoke::attach_to_cart(rc, |s| YokeableStr(&s[start..end])))
    }

    pub fn backing_rc(&self) -> &Rc<str> {
        self.0.backing_cart()
    }
}

impl From<Rc<str>> for OwnedStr {
    fn from(rc: Rc<str>) -> Self {
        Self(Yoke::attach_to_cart(rc, |s| YokeableStr(s)))
    }
}

impl From<String> for OwnedStr {
    fn from(s: String) -> Self {
        Self::from(Rc::<str>::from(s))
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

#[derive(Yokeable)]
struct YokeablePropId<'a>(PropertyId<'a>);

pub struct OwnedPropId(Yoke<YokeablePropId<'static>, Rc<str>>);

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
        Self(Yoke::attach_to_cart(rc, move |full| {
            YokeablePropId(PropertyId::from(&full[start..end]))
        }))
    }
}

impl From<&str> for OwnedPropId {
    fn from(value: &str) -> Self {
        let owned_str = OwnedStr::from(value.to_string());
        OwnedPropId::from(&owned_str)
    }
}

impl OwnedPropId {
    pub fn inner(&self) -> &PropertyId<'_> {
        &self.0.get().0
    }

    pub fn as_str(&self) -> &str {
        self.0.backing_cart().as_ref()
    }
}
