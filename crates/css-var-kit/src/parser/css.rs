use std::{cell::OnceCell, path::PathBuf, rc::Rc};

use lightningcss::{
    properties::custom::TokenList, stylesheet::ParserOptions, traits::ParseWithOptions,
};

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
    fn new(raw: OwnedStr, offset: usize, line: u32, column: u32) -> Self {
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
    pub file_path: Rc<PathBuf>,
    pub source: OwnedStr,
    pub ident: PropertyIdent,
    pub value: PropertyValue,
    pub ignore_comments: Vec<OwnedStr>,
    pub token_list: OnceCell<OwnedTokenList>,
}

impl Property {
    pub fn token_list(&self) -> &OwnedTokenList {
        self.token_list.get_or_init(|| {
            OwnedTokenList::parse(&self.value.raw).unwrap_or(OwnedTokenList::default())
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub file_path: Rc<PathBuf>,
    pub properties: Vec<Property>,
}

struct Scanner<'a> {
    bytes: &'a [u8],
    pos: usize,
    line: u32,
    col: u32,
}

impl<'a> Scanner<'a> {
    fn new_with_offset(css: &'a OwnedStr, line_offset: u32, column_offset: u32) -> Self {
        Self {
            bytes: css.as_bytes(),
            pos: 0,
            line: line_offset,
            col: column_offset,
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    fn advance(&mut self, times: usize) {
        for _ in 0..times {
            if self.pos < self.bytes.len() {
                match self.bytes[self.pos] {
                    b'\n' => {
                        // For \r\n, \r already incremented line — skip the duplicate
                        if self.pos == 0 || self.bytes[self.pos - 1] != b'\r' {
                            self.line += 1;
                        }
                        self.col = 0;
                    }
                    b'\r' => {
                        self.line += 1;
                        self.col = 0;
                    }
                    _ => {
                        self.col += 1;
                    }
                }
                self.pos += 1;
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.bytes.len()
            && (self.bytes[self.pos] == b' ' || self.bytes[self.pos] == b'\t')
        {
            self.advance(1);
        }
    }

    fn skip_escape(&mut self) {
        self.advance(1); // skip '\'
        if self.is_eof() {
            return;
        }
        if self.bytes[self.pos].is_ascii_hexdigit() {
            let mut count = 0;
            while !self.is_eof() && count < 6 && self.bytes[self.pos].is_ascii_hexdigit() {
                self.advance(1);
                count += 1;
            }
            // Consume optional trailing whitespace (single space/tab/newline)
            if !self.is_eof() && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\n' | b'\r') {
                self.advance(1);
            }
        } else if self.bytes[self.pos] != b'\n' && self.bytes[self.pos] != b'\r' {
            self.advance(1);
        }
    }

    fn skip_string_literal(&mut self) {
        let quote = self.bytes[self.pos];
        self.advance(1);
        while !self.is_eof() {
            if self.bytes[self.pos] == b'\\' {
                self.advance(2);
            } else if self.bytes[self.pos] == quote {
                self.advance(1);
                break;
            } else {
                self.advance(1);
            }
        }
    }

    fn skip_comment(&mut self) {
        self.advance(2); // skip /*
        while !self.is_eof() {
            if self.bytes[self.pos] == b'*' && self.peek_at(1) == Some(b'/') {
                self.advance(2);
                return;
            }
            self.advance(1);
        }
    }

    fn scan_comment(&mut self, css: &'a OwnedStr) -> OwnedStr {
        self.advance(2); // skip /*
        let content_start = self.pos;
        while !self.is_eof() {
            if self.bytes[self.pos] == b'*' && self.peek_at(1) == Some(b'/') {
                let content_end = self.pos;
                self.advance(2);
                return css.map(|s| s[content_start..content_end].trim());
            }
            self.advance(1);
        }
        // Unterminated comment
        css.map(|s| s[content_start..self.pos].trim())
    }

    fn skip_at_rule(&mut self) {
        self.advance(1); // skip '@'
        // Skip until ';' (statement) or matched '{...}' (block)
        while !self.is_eof() {
            match self.bytes[self.pos] {
                b'"' | b'\'' => self.skip_string_literal(),
                b'/' if self.peek_at(1) == Some(b'*') => self.skip_comment(),
                b';' => {
                    self.advance(1);
                    return;
                }
                b'{' => {
                    // Skip the block including nested braces
                    self.advance(1);
                    let mut depth = 1i32;
                    while !self.is_eof() && depth > 0 {
                        match self.bytes[self.pos] {
                            b'"' | b'\'' => self.skip_string_literal(),
                            b'/' if self.peek_at(1) == Some(b'*') => self.skip_comment(),
                            b'{' => {
                                depth += 1;
                                self.advance(1);
                            }
                            b'}' => {
                                depth -= 1;
                                self.advance(1);
                            }
                            _ => self.advance(1),
                        }
                    }
                    return;
                }
                _ => self.advance(1),
            }
        }
    }

    fn scan_value_end(&mut self) -> usize {
        let mut paren_depth = 0i32;

        while !self.is_eof() {
            match self.bytes[self.pos] {
                // Skip string literals inside values
                b'"' | b'\'' => {
                    self.skip_string_literal();
                }
                b'(' => {
                    paren_depth += 1;
                    self.advance(1);
                }
                b')' => {
                    paren_depth -= 1;
                    self.advance(1);
                }
                b';' if paren_depth <= 0 => {
                    let end = self.pos;
                    self.advance(1); // skip ';'
                    return end;
                }
                b'}' if paren_depth <= 0 => {
                    // Don't consume '}', let the main loop handle brace_depth
                    return self.pos;
                }
                b'/' if self.peek_at(1) == Some(b'*') && paren_depth <= 0 => {
                    // Comment starts — value ends here
                    let end = self.pos;
                    // Skip the comment
                    self.skip_comment();
                    // After the comment, continue scanning for the actual terminator (`;` or `}`)
                    while !self.is_eof() {
                        match self.bytes[self.pos] {
                            b';' => {
                                self.advance(1);
                                break;
                            }
                            b'}' => break,
                            b' ' | b'\t' | b'\n' | b'\r' => {
                                self.advance(1);
                            }
                            _ => {
                                // Something else after the comment, stop
                                break;
                            }
                        }
                    }
                    return end;
                }
                b'\n' | b'\r' if paren_depth <= 0 => {
                    // Check if the next non-whitespace looks like a new property
                    let mut skip = 1;
                    // For \r\n, skip both bytes
                    if self.bytes[self.pos] == b'\r' && self.peek_at(1) == Some(b'\n') {
                        skip = 2;
                    }
                    let after_newline = &self.bytes[self.pos + skip..];
                    let trimmed_offset = after_newline
                        .iter()
                        .position(|&b| b != b' ' && b != b'\t')
                        .unwrap_or(after_newline.len());
                    let rest = &self.bytes[self.pos + skip + trimmed_offset..];

                    if looks_like_property_start(rest) {
                        let end = self.pos;
                        self.advance(skip);
                        return end;
                    }
                    self.advance(1);
                }
                _ => {
                    self.advance(1);
                }
            }
        }

        // EOF — value extends to end
        self.bytes.len()
    }
}

pub fn parse(css: OwnedStr, file_path: Rc<PathBuf>) -> ParseResult {
    parse_impl(css.clone(), css, file_path, 0, 0, 0, 0)
}

/// Used when parsing `<style>` blocks from HTML-like files.
/// `full_source` is the entire file content stored in `Property.source`.
/// `line_offset`/`column_offset` are the absolute start position of the CSS content.
/// `byte_offset` is added to `Property.name.offset` and `Property.value.offset`.
pub fn parse_with_offset(
    css: OwnedStr,
    file_path: Rc<PathBuf>,
    full_source: OwnedStr,
    line_offset: u32,
    column_offset: u32,
    byte_offset: usize,
) -> ParseResult {
    parse_impl(
        css,
        full_source,
        file_path,
        0,
        line_offset,
        column_offset,
        byte_offset,
    )
}

fn parse_impl(
    css: OwnedStr,
    source: OwnedStr,
    file_path: Rc<PathBuf>,
    initial_brace_depth: i32,
    line_offset: u32,
    column_offset: u32,
    byte_offset: usize,
) -> ParseResult {
    let mut s = Scanner::new_with_offset(&css, line_offset, column_offset);
    let mut properties = Vec::new();
    let mut pending_ignores: Vec<OwnedStr> = Vec::new();
    let mut last_comment_end_line: u32 = 0;

    let mut brace_depth = initial_brace_depth;

    while !s.is_eof() {
        match s.bytes[s.pos] {
            // Skip string literals
            b'"' | b'\'' => {
                s.skip_string_literal();
            }
            // Scan comments for cvk-ignore directives
            b'/' if s.peek_at(1) == Some(b'*') => {
                let comment_start_line = s.line;
                // Blank line between previous comment and this one breaks the chain
                if !pending_ignores.is_empty() && comment_start_line > last_comment_end_line + 1 {
                    pending_ignores.clear();
                }
                let content = s.scan_comment(&css);
                last_comment_end_line = s.line;
                if content.starts_with("cvk-ignore") {
                    pending_ignores.push(content);
                }
            }
            // Skip @-rules at top level (e.g. @property, @import, @charset)
            b'@' if brace_depth == initial_brace_depth => {
                pending_ignores.clear();
                s.skip_at_rule();
            }
            b'{' => {
                pending_ignores.clear();
                brace_depth += 1;
                s.advance(1);
            }
            b'}' => {
                pending_ignores.clear();
                brace_depth -= 1;
                s.advance(1);
            }
            _ if brace_depth > 0 && is_ident_start(s.bytes[s.pos]) => {
                // Try to parse a property name
                let name_start = s.pos;
                let name_line = s.line;
                let name_col = s.col;

                // Blank line between last comment and property breaks the chain
                if !pending_ignores.is_empty() && name_line > last_comment_end_line + 1 {
                    pending_ignores.clear();
                }

                // Read the identifier (including escape sequences and non-ASCII)
                while !s.is_eof() {
                    if s.bytes[s.pos] == b'\\' {
                        s.skip_escape();
                    } else if is_ident_char(s.bytes[s.pos]) {
                        s.advance(1);
                    } else {
                        break;
                    }
                }
                let name_end = s.pos;

                // Skip whitespace between name and ':'
                s.skip_whitespace();

                if !s.is_eof() && s.bytes[s.pos] == b':' {
                    s.advance(1); // skip ':'

                    // Skip whitespace after ':'
                    s.skip_whitespace();

                    let value_line = s.line;
                    let value_col = s.col;
                    let value_start = s.pos;

                    // Read value until end
                    let value_end = s.scan_value_end();

                    let raw_value = css.map(|s| s[value_start..value_end].trim());

                    let ignore_comments = std::mem::take(&mut pending_ignores);
                    let raw_name = css.slice(name_start..name_end);
                    properties.push(Property {
                        file_path: file_path.clone(),
                        source: source.clone(),
                        ident: PropertyIdent::new(
                            raw_name,
                            name_start + byte_offset,
                            name_line,
                            name_col,
                        ),
                        value: PropertyValue {
                            raw: raw_value,
                            offset: value_start + byte_offset,
                            line: value_line,
                            column: value_col,
                        },
                        ignore_comments,
                    });
                }
                // If no ':', it's not a property (e.g. a selector), just continue
            }
            _ => {
                s.advance(1);
            }
        }
    }

    ParseResult {
        file_path,
        properties,
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'-' || b == b'_' || b == b'\\' || b >= 0x80
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80
}

fn skip_escape_bytes(bytes: &[u8], pos: usize) -> usize {
    let mut j = pos + 1; // skip '\'
    if j >= bytes.len() {
        return j;
    }
    if bytes[j].is_ascii_hexdigit() {
        let start = j;
        while j < bytes.len() && j - start < 6 && bytes[j].is_ascii_hexdigit() {
            j += 1;
        }
        if j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\n' | b'\r') {
            j += 1;
        }
    } else if bytes[j] != b'\n' && bytes[j] != b'\r' {
        j += 1;
    }
    j
}

fn looks_like_property_start(bytes: &[u8]) -> bool {
    if bytes.is_empty() || !is_ident_start(bytes[0]) {
        return false;
    }
    let mut j = 0;
    while j < bytes.len() {
        if bytes[j] == b'\\' {
            j = skip_escape_bytes(bytes, j);
        } else if is_ident_char(bytes[j]) {
            j += 1;
        } else {
            break;
        }
    }
    // Skip whitespace
    while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
        j += 1;
    }
    j < bytes.len() && bytes[j] == b':'
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
    use std::path::PathBuf;

    use lightningcss::properties::PropertyId;

    use super::*;

    const TEST_PATH: &str = "test.css";

    fn test_parse(css: &str) -> ParseResult {
        parse(OwnedStr::from(css), Rc::new(PathBuf::from(TEST_PATH)))
    }

    fn map_owned_str(vec: Vec<&str>) -> Vec<OwnedStr> {
        vec.into_iter()
            .map(|s| OwnedStr::from(s.to_string()))
            .collect()
    }

    #[test]
    fn basic_var_def() {
        let css = ":root {\n    --main-color: red;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(
            result.properties[0].ident.raw.as_ref() as &str,
            "--main-color"
        );
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
        assert_eq!(result.properties[0].ident.line, 1);
    }

    #[test]
    fn multiple_var_defs() {
        let css = ":root {\n    --color: red;\n    --size: 16px;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "--color");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "--size");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "16px");
    }

    #[test]
    fn single_line_multiple_defs() {
        let css = ":root { --a: 1; --b: 2; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "--a");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "1");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "--b");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "2");
    }

    #[test]
    fn incomplete_input() {
        let css = ":root {\n    --color: red";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
    }

    #[test]
    fn regular_property() {
        let css = ".btn { color: red; font-size: 16px; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "color");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "font-size");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "16px");
    }

    #[test]
    fn var_usage_in_value() {
        let css = ".btn { color: var(--main-color); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "color");
        assert_eq!(
            result.properties[0].value.raw.as_ref() as &str,
            "var(--main-color)"
        );
    }

    #[test]
    fn def_and_regular_property() {
        let css = ":root { --c: red; }\n.a { color: var(--c); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "--c");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "color");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "var(--c)");
    }

    #[test]
    fn property_after_string_literal() {
        let css = ".a::before { content: \"hello\"; color: var(--c); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "content");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "\"hello\"");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "color");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "var(--c)");
    }

    #[test]
    fn ignore_content_in_comment() {
        let css = ".a { /* --not-a-var */ color: var(--c); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "color");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "var(--c)");
    }

    #[test]
    fn strip_inline_comment_from_value() {
        let css = ":root {\n    --color: red /* main color */;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
    }

    #[test]
    fn strip_inline_comment_no_semicolon() {
        let css = ":root {\n    --color: red /* main color */\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
    }

    #[test]
    fn missing_semicolon_next_property() {
        let css = ":root {\n    --a: red\n    --b: blue;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "--a");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "--b");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "blue");
    }

    #[test]
    fn missing_semicolon_regular_property() {
        let css = ".a {\n    --color: red\n    font-size: 16px;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "--color");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "font-size");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "16px");
    }

    #[test]
    fn missing_semicolon_cr_only() {
        // standalone \r should also trigger missing-semicolon detection
        let css = ":root {\r    --a: red\r    --b: blue;\r}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "--a");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "--b");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "blue");
    }

    #[test]
    fn ignore_content_in_multiline_comment() {
        let css = "/*\n  --not-a-var: red;\n*/\n.a { color: var(--c); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "color");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "var(--c)");
    }

    #[test]
    fn multiline_function_value() {
        let css = ".a {\n    background: linear-gradient(\n        red,\n        blue\n    );\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(
            result.properties[0].ident.raw.as_ref() as &str,
            "background"
        );
        assert_eq!(
            result.properties[0].value.raw.as_ref() as &str,
            "linear-gradient(\n        red,\n        blue\n    )"
        );
    }

    #[test]
    fn paren_depth_nested() {
        let css = ".a { color: rgb(calc(100 + 50), 0, 0); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "color");
        assert_eq!(
            result.properties[0].value.raw.as_ref() as &str,
            "rgb(calc(100 + 50), 0, 0)"
        );
    }

    #[test]
    fn value_position_tracking() {
        let css = ":root {\n    --color: red;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties[0].ident.offset, 12);
        assert_eq!(result.properties[0].ident.line, 1);
        assert_eq!(result.properties[0].ident.column, 4);
        assert_eq!(result.properties[0].value.offset, 21);
        assert_eq!(result.properties[0].value.line, 1);
        assert_eq!(result.properties[0].value.column, 13);
    }

    #[test]
    fn crlf_position_tracking() {
        // \r\n should be treated as a single newline, same positions as \n
        // compared to \n version: each \r\n adds 1 extra byte
        let css = ":root {\r\n    --color: red;\r\n}";
        let result = test_parse(css);
        assert_eq!(result.properties[0].ident.offset, 13);
        assert_eq!(result.properties[0].ident.line, 1);
        assert_eq!(result.properties[0].ident.column, 4);
        assert_eq!(result.properties[0].value.offset, 22);
        assert_eq!(result.properties[0].value.line, 1);
        assert_eq!(result.properties[0].value.column, 13);
    }

    #[test]
    fn multibyte_char_column_is_byte_based() {
        // Scanner column is byte-based; "あ" is 3 bytes in UTF-8
        let css = ".あ { --color: red; }";
        let result = test_parse(css);
        assert_eq!(result.properties[0].ident.line, 0);
        // ".あ { " = 1 + 3 + 1 + 1 + 1 = 7 bytes
        assert_eq!(result.properties[0].ident.column, 7);
    }

    #[test]
    fn standalone_cr_position_tracking() {
        // standalone \r (old Mac) should also be treated as a newline
        let css = ":root {\r    --color: red;\r}";
        let result = test_parse(css);
        assert_eq!(result.properties[0].ident.offset, 12);
        assert_eq!(result.properties[0].ident.line, 1);
        assert_eq!(result.properties[0].ident.column, 4);
        assert_eq!(result.properties[0].value.offset, 21);
        assert_eq!(result.properties[0].value.line, 1);
        assert_eq!(result.properties[0].value.column, 13);
    }

    #[test]
    fn parse_ignores_bare_properties() {
        let result = test_parse("color: red; font-size: 16px;");
        assert_eq!(result.properties.len(), 0);
    }

    // Edge cases

    #[test]
    fn semicolon_inside_string_value() {
        let css = ".a { content: \"a;b\"; color: red; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "content");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "\"a;b\"");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "color");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "red");
    }

    #[test]
    fn closing_brace_inside_string_value() {
        let css = ".a { content: \"}\"; color: red; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(
            result.properties[0].ident.raw.as_ref() as &str as &str,
            "content"
        );
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "\"}\"");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "color");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "red");
    }

    #[test]
    fn escaped_quote_in_string_value() {
        let css = r#".a { content: "he said \"hello\""; }"#;
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "content");
        assert_eq!(
            result.properties[0].value.raw.as_ref() as &str,
            r#""he said \"hello\"""#
        );
    }

    #[test]
    fn unterminated_string_at_eof() {
        let css = ".a { content: \"hello";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "content");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "\"hello");
    }

    #[test]
    fn unterminated_comment() {
        let css = ".a { color: red; } /* never closed";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "color");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
    }

    #[test]
    fn unterminated_comment_inside_value() {
        let css = ".a { color: red /* unclosed";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
    }

    #[test]
    fn empty_value() {
        let css = ".a { --empty: ; --next: blue; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "--empty");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "--next");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "blue");
    }

    #[test]
    fn no_spaces_around_colon() {
        let css = ".a{color:red;font-size:16px}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "color");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "font-size");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "16px");
    }

    #[test]
    fn nested_blocks() {
        let css = ".a { .b { color: red; } font-size: 16px; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "color");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "font-size");
        assert_eq!(result.properties[1].value.raw.as_ref() as &str, "16px");
    }

    #[test]
    fn backslash_at_eof_in_string() {
        let css = ".a { content: \"test\\";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "content");
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "\"test\\");
    }

    #[test]
    fn value_terminated_by_closing_brace() {
        let css = ".a { color: red }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
    }

    #[test]
    fn at_property_rule_skipped() {
        let css = "@property --my-color {\n  syntax: \"<color>\";\n  inherits: false;\n  initial-value: red;\n}\n.a { color: var(--my-color); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "color");
        assert_eq!(
            result.properties[0].value.raw.as_ref() as &str,
            "var(--my-color)"
        );
    }

    #[test]
    fn at_property_between_selectors() {
        let css = ".before { margin: 0; }\n@property --x {\n  syntax: \"*\";\n  inherits: true;\n}\n.after { padding: 0; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "margin");
        assert_eq!(result.properties[1].ident.raw.as_ref() as &str, "padding");
    }

    #[test]
    fn at_import_skipped() {
        let css = "@import url(\"reset.css\");\n.a { color: red; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "color");
    }

    #[test]
    fn unterminated_comment_consumes_all_bytes() {
        let str = OwnedStr::from("/* {");
        let mut s = Scanner::new_with_offset(&str, 0, 0);
        s.skip_comment();
        assert!(
            s.is_eof(),
            "skip_comment should consume all bytes of an unterminated comment, but pos={} len={}",
            s.pos,
            s.bytes.len()
        );
    }

    // cvk-ignore tests

    #[test]
    fn cvk_ignore_sets_ignore_comments() {
        let css = ".a {\n    /* cvk-ignore */\n    color: var(--c);\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(
            result.properties[0].ignore_comments,
            map_owned_str(vec!["cvk-ignore"])
        );
    }

    #[test]
    fn no_cvk_ignore_leaves_empty() {
        let css = ".a {\n    /* just a comment */\n    color: var(--c);\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert!(result.properties[0].ignore_comments.is_empty());
    }

    #[test]
    fn cvk_ignore_with_rule_name() {
        let css = ".a {\n    /* cvk-ignore: no-undefined-variable-use */\n    color: var(--c);\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(
            result.properties[0].ignore_comments,
            map_owned_str(vec!["cvk-ignore: no-undefined-variable-use"])
        );
    }

    #[test]
    fn cvk_ignore_persists_through_other_comments() {
        let css =
            ".a {\n    /* cvk-ignore */\n    /* stylelint-disable */\n    color: var(--c);\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(
            result.properties[0].ignore_comments,
            map_owned_str(vec!["cvk-ignore"])
        );
    }

    #[test]
    fn multiple_cvk_ignore_comments() {
        let css =
            ".a {\n    /* cvk-ignore */\n    /* cvk-ignore: rule-a */\n    color: var(--c);\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(
            result.properties[0].ignore_comments,
            map_owned_str(vec!["cvk-ignore", "cvk-ignore: rule-a"])
        );
    }

    #[test]
    fn cvk_ignore_resets_after_brace() {
        let css = "/* cvk-ignore */\n.a {\n    color: var(--c);\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert!(result.properties[0].ignore_comments.is_empty());
    }

    #[test]
    fn cvk_ignore_resets_after_closing_brace() {
        let css = ".a { /* cvk-ignore */ } .b { color: var(--c); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert!(result.properties[0].ignore_comments.is_empty());
    }

    #[test]
    fn cvk_ignore_only_applies_to_next_property() {
        let css = ".a {\n    /* cvk-ignore */\n    color: var(--c);\n    font-size: var(--s);\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(
            result.properties[0].ignore_comments,
            map_owned_str(vec!["cvk-ignore"])
        );
        assert!(result.properties[1].ignore_comments.is_empty());
    }

    #[test]
    fn cvk_ignore_resets_after_at_rule() {
        let css = "/* cvk-ignore */\n@import url(\"reset.css\");\n.a { color: var(--c); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert!(result.properties[0].ignore_comments.is_empty());
    }

    #[test]
    fn cvk_ignore_resets_after_blank_line() {
        let css = ".a {\n    /* cvk-ignore */\n\n    color: var(--c);\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert!(result.properties[0].ignore_comments.is_empty());
    }

    #[test]
    fn cvk_ignore_blank_line_between_comments() {
        let css = ".a {\n    /* cvk-ignore */\n\n    /* other comment */\n    color: var(--c);\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert!(result.properties[0].ignore_comments.is_empty());
    }

    #[test]
    fn cvk_ignore_no_blank_line_applies() {
        let css = ".a {\n    /* cvk-ignore */\n    color: var(--c);\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(
            result.properties[0].ignore_comments,
            map_owned_str(vec!["cvk-ignore"])
        );
    }

    // CSS escape sequence tests

    // #[test]
    // fn hex_escape_in_property_name() {
    //     // col\6fr → color (0x6f = 'o')
    //     let css = ".a { col\\6fr: red; }";
    //     let result = test_parse(css);
    //     assert_eq!(result.properties.len(), 1);
    //     assert_eq!(result.properties[0].ident.raw, "col\\6fr");
    //     assert_eq!(&*result.properties[0].ident.unescaped, "color");
    // }

    // #[test]
    // fn hex_escape_with_trailing_space() {
    //     // col\\6f r → color (trailing space consumed by escape)
    //     let css = ".a { col\\6f r: red; }";
    //     let result = test_parse(css);
    //     assert_eq!(result.properties.len(), 1);
    //     assert_eq!(&*result.properties[0].ident.unescaped, "color");
    // }

    // #[test]
    // fn literal_escape_in_property_name() {
    //     // \.a is a selector escape, test in property context:
    //     // --my\-var → --my-var
    //     let css = ".a { --my\\-var: red; }";
    //     let result = test_parse(css);
    //     assert_eq!(result.properties.len(), 1);
    //     assert_eq!(result.properties[0].ident.raw, "--my\\-var");
    //     assert_eq!(&*result.properties[0].ident.unescaped, "--my-var");
    // }

    #[test]
    fn escaped_property_matches_property_id() {
        // col\6fr should produce the same PropertyId as color
        let css = ".a { col\\6fr: red; }";
        let result = test_parse(css);
        assert_eq!(
            result.properties[0].ident.property_id.inner(),
            &PropertyId::from("color")
        );
    }

    // #[test]
    // fn escaped_custom_property_matches_property_id() {
    //     let css = ".a { --my\\-color: red; }";
    //     let result = test_parse(css);
    //     assert_eq!(
    //         result.properties[0].ident.property_id,
    //         PropertyId::from("--my-color")
    //     );
    // }

    // #[test]
    // fn no_escape_leaves_unescaped_borrowed() {
    //     let css = ".a { color: red; }";
    //     let result = test_parse(css);
    //     assert!(matches!(
    //         result.properties[0].ident.unescaped,
    //         Cow::Borrowed(_)
    //     ));
    // }

    // #[test]
    // fn escape_produces_owned_unescaped() {
    //     let css = ".a { col\\6fr: red; }";
    //     let result = test_parse(css);
    //     assert!(matches!(
    //         result.properties[0].ident.unescaped,
    //         Cow::Owned(_)
    //     ));
    // }

    #[test]
    fn non_ascii_identifier() {
        let css = ".a { --あいろ: red; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].ident.raw.as_ref() as &str, "--あいろ");
    }

    #[test]
    fn looks_like_property_start_with_escape() {
        // Missing semicolon recovery should work with escaped identifiers
        let css = ".a {\n  color: red\n  col\\6fr: blue;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].value.raw.as_ref() as &str, "red");
        assert_eq!(&*result.properties[1].ident.raw.as_ref() as &str, "color");
    }

    #[test]
    fn unescape_css_ident_no_escape() {
        assert!(matches!(unescape_css_ident("color"), None));
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
