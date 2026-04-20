use std::{cell::OnceCell, path::Path, rc::Rc};

use crate::owned::{OwnedPropId, OwnedStr, OwnedTokenList};

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyIdent {
    pub raw: OwnedStr,
    pub property_id: OwnedPropId,
    pub offset: usize,
    pub line: u32,
    pub column: u32,
}

impl PropertyIdent {
    pub(in crate::parser) fn new(raw: OwnedStr, offset: usize, line: u32, column: u32) -> Self {
        let property_id = if let Some(unescaped) = unescape_css_ident(raw.as_ref()) {
            OwnedPropId::from(unescaped)
        } else {
            OwnedPropId::from(&raw)
        };

        Self {
            raw,
            property_id,
            offset,
            line,
            column,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyValue {
    pub raw: OwnedStr,
    pub offset: usize,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub file_path: Rc<Path>,
    pub source: OwnedStr,
    pub ident: PropertyIdent,
    pub value: PropertyValue,
    pub ignore_comments: Vec<OwnedStr>,
    pub token_list: OnceCell<OwnedTokenList>,
}

impl Property {
    pub fn token_list(&self) -> &OwnedTokenList {
        self.token_list
            .get_or_init(|| OwnedTokenList::parse(&self.value.raw).unwrap_or_default())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub file_path: Rc<Path>,
    pub properties: Vec<Property>,
}

fn unescape_css_ident(raw: &str) -> Option<String> {
    if !raw.contains('\\') {
        return None;
    }
    let bytes = raw.as_bytes();
    let mut result = String::with_capacity(raw.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 1;
            if i >= bytes.len() {
                break;
            }
            if bytes[i].is_ascii_hexdigit() {
                let start = i;
                while i < bytes.len() && i - start < 6 && bytes[i].is_ascii_hexdigit() {
                    i += 1;
                }
                if let Ok(cp) = u32::from_str_radix(&raw[start..i], 16) {
                    if let Some(c) = char::from_u32(cp) {
                        result.push(c);
                    }
                }
                // Consume optional trailing whitespace
                if i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
                    i += 1;
                }
            } else if bytes[i] != b'\n' && bytes[i] != b'\r' {
                let rest = &raw[i..];
                let c = rest.chars().next().unwrap();
                result.push(c);
                i += c.len_utf8();
            }
        } else {
            let rest = &raw[i..];
            let c = rest.chars().next().unwrap();
            result.push(c);
            i += c.len_utf8();
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescape_css_ident_no_escape() {
        assert!(unescape_css_ident("color").is_none());
    }

    #[test]
    fn unescape_css_ident_hex() {
        assert_eq!(unescape_css_ident("col\\6fr"), Some("color".to_string()));
    }

    #[test]
    fn unescape_css_ident_hex_with_space() {
        assert_eq!(unescape_css_ident("col\\6f r"), Some("color".to_string()));
    }

    #[test]
    fn unescape_css_ident_literal() {
        assert_eq!(unescape_css_ident("my\\-var"), Some("my-var".to_string()));
    }

    #[test]
    fn unescape_css_ident_unicode() {
        // \3042 = 'あ' (U+3042)
        assert_eq!(unescape_css_ident("\\3042"), Some("あ".to_string()));
    }
}
