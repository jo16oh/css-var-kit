use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyIdent<'a> {
    pub raw: &'a str,
    pub offset: usize,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyValue<'a> {
    pub raw: &'a str,
    pub offset: usize,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property<'a> {
    pub file_path: &'a Path,
    pub name: PropertyIdent<'a>,
    pub value: PropertyValue<'a>,
}

#[derive(Debug, PartialEq)]
pub struct ParseResult<'a> {
    pub file_path: &'a Path,
    pub properties: Vec<Property<'a>>,
}

struct Scanner<'a> {
    bytes: &'a [u8],
    pos: usize,
    line: u32,
    col: u32,
}

impl<'a> Scanner<'a> {
    fn new(css: &'a str) -> Self {
        Self {
            bytes: css.as_bytes(),
            pos: 0,
            line: 0,
            col: 0,
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
                    if self.bytes[self.pos] == b'\r'
                        && self.peek_at(1) == Some(b'\n')
                    {
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

pub fn parse<'a>(css: &'a str, file_path: &'a Path) -> ParseResult<'a> {
    parse_impl(css, file_path, 0)
}

pub fn parse_style_attr<'a>(css: &'a str, file_path: &'a Path) -> ParseResult<'a> {
    parse_impl(css, file_path, 1)
}

fn parse_impl<'a>(css: &'a str, file_path: &'a Path, initial_brace_depth: i32) -> ParseResult<'a> {
    let mut s = Scanner::new(css);
    let mut properties = Vec::new();

    let mut brace_depth = initial_brace_depth;

    while !s.is_eof() {
        match s.bytes[s.pos] {
            // Skip string literals
            b'"' | b'\'' => {
                s.skip_string_literal();
            }
            // Skip comments
            b'/' if s.peek_at(1) == Some(b'*') => {
                s.skip_comment();
            }
            b'{' => {
                brace_depth += 1;
                s.advance(1);
            }
            b'}' => {
                brace_depth -= 1;
                s.advance(1);
            }
            _ if brace_depth > 0 && is_ident_start(s.bytes[s.pos]) => {
                // Try to parse a property name
                let name_start = s.pos;
                let name_line = s.line;
                let name_col = s.col;

                // Read the identifier
                while !s.is_eof() && is_ident_char(s.bytes[s.pos]) {
                    s.advance(1);
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

                    let raw_value = css[value_start..value_end].trim();

                    properties.push(Property {
                        file_path,
                        name: PropertyIdent {
                            raw: &css[name_start..name_end],
                            offset: name_start,
                            line: name_line,
                            column: name_col,
                        },
                        value: PropertyValue {
                            raw: raw_value,
                            offset: value_start,
                            line: value_line,
                            column: value_col,
                        },
                    });
                }
                // If no ':', it's not a property (e.g. a selector), just continue
            }
            _ => {
                s.advance(1);
            }
        }
    }

    ParseResult { file_path, properties }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'-' || b == b'_'
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// Check if the bytes at this position look like the start of a new property (ident followed by ':').
fn looks_like_property_start(bytes: &[u8]) -> bool {
    if bytes.is_empty() || !is_ident_start(bytes[0]) {
        return false;
    }
    let mut j = 0;
    while j < bytes.len() && is_ident_char(bytes[j]) {
        j += 1;
    }
    // Skip whitespace
    while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
        j += 1;
    }
    j < bytes.len() && bytes[j] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PATH: &str = "test.css";

    fn test_parse(css: &str) -> ParseResult<'_> {
        parse(css, Path::new(TEST_PATH))
    }

    fn test_parse_style_attr(css: &str) -> ParseResult<'_> {
        parse_style_attr(css, Path::new(TEST_PATH))
    }

    #[test]
    fn basic_var_def() {
        let css = ":root {\n    --main-color: red;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].name.raw, "--main-color");
        assert_eq!(result.properties[0].value.raw, "red");
        assert_eq!(result.properties[0].name.line, 1);
    }

    #[test]
    fn multiple_var_defs() {
        let css = ":root {\n    --color: red;\n    --size: 16px;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "--color");
        assert_eq!(result.properties[0].value.raw, "red");
        assert_eq!(result.properties[1].name.raw, "--size");
        assert_eq!(result.properties[1].value.raw, "16px");
    }

    #[test]
    fn single_line_multiple_defs() {
        let css = ":root { --a: 1; --b: 2; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "--a");
        assert_eq!(result.properties[0].value.raw, "1");
        assert_eq!(result.properties[1].name.raw, "--b");
        assert_eq!(result.properties[1].value.raw, "2");
    }

    #[test]
    fn incomplete_input() {
        let css = ":root {\n    --color: red";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw, "red");
    }

    #[test]
    fn regular_property() {
        let css = ".btn { color: red; font-size: 16px; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "color");
        assert_eq!(result.properties[0].value.raw, "red");
        assert_eq!(result.properties[1].name.raw, "font-size");
        assert_eq!(result.properties[1].value.raw, "16px");
    }

    #[test]
    fn var_usage_in_value() {
        let css = ".btn { color: var(--main-color); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].name.raw, "color");
        assert_eq!(result.properties[0].value.raw, "var(--main-color)");
    }

    #[test]
    fn def_and_regular_property() {
        let css = ":root { --c: red; }\n.a { color: var(--c); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "--c");
        assert_eq!(result.properties[0].value.raw, "red");
        assert_eq!(result.properties[1].name.raw, "color");
        assert_eq!(result.properties[1].value.raw, "var(--c)");
    }

    #[test]
    fn property_after_string_literal() {
        let css = ".a::before { content: \"hello\"; color: var(--c); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "content");
        assert_eq!(result.properties[0].value.raw, "\"hello\"");
        assert_eq!(result.properties[1].name.raw, "color");
        assert_eq!(result.properties[1].value.raw, "var(--c)");
    }

    #[test]
    fn ignore_content_in_comment() {
        let css = ".a { /* --not-a-var */ color: var(--c); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].name.raw, "color");
        assert_eq!(result.properties[0].value.raw, "var(--c)");
    }

    #[test]
    fn strip_inline_comment_from_value() {
        let css = ":root {\n    --color: red /* main color */;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw, "red");
    }

    #[test]
    fn strip_inline_comment_no_semicolon() {
        let css = ":root {\n    --color: red /* main color */\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw, "red");
    }

    #[test]
    fn missing_semicolon_next_property() {
        let css = ":root {\n    --a: red\n    --b: blue;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "--a");
        assert_eq!(result.properties[0].value.raw, "red");
        assert_eq!(result.properties[1].name.raw, "--b");
        assert_eq!(result.properties[1].value.raw, "blue");
    }

    #[test]
    fn missing_semicolon_regular_property() {
        let css = ".a {\n    --color: red\n    font-size: 16px;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "--color");
        assert_eq!(result.properties[0].value.raw, "red");
        assert_eq!(result.properties[1].name.raw, "font-size");
        assert_eq!(result.properties[1].value.raw, "16px");
    }

    #[test]
    fn missing_semicolon_cr_only() {
        // standalone \r should also trigger missing-semicolon detection
        let css = ":root {\r    --a: red\r    --b: blue;\r}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "--a");
        assert_eq!(result.properties[0].value.raw, "red");
        assert_eq!(result.properties[1].name.raw, "--b");
        assert_eq!(result.properties[1].value.raw, "blue");
    }

    #[test]
    fn ignore_content_in_multiline_comment() {
        let css = "/*\n  --not-a-var: red;\n*/\n.a { color: var(--c); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].name.raw, "color");
        assert_eq!(result.properties[0].value.raw, "var(--c)");
    }

    #[test]
    fn multiline_function_value() {
        let css = ".a {\n    background: linear-gradient(\n        red,\n        blue\n    );\n}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].name.raw, "background");
        assert_eq!(
            result.properties[0].value.raw,
            "linear-gradient(\n        red,\n        blue\n    )"
        );
    }

    #[test]
    fn paren_depth_nested() {
        let css = ".a { color: rgb(calc(100 + 50), 0, 0); }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].name.raw, "color");
        assert_eq!(result.properties[0].value.raw, "rgb(calc(100 + 50), 0, 0)");
    }

    #[test]
    fn value_position_tracking() {
        let css = ":root {\n    --color: red;\n}";
        let result = test_parse(css);
        assert_eq!(result.properties[0].name.offset, 12);
        assert_eq!(result.properties[0].name.line, 1);
        assert_eq!(result.properties[0].name.column, 4);
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
        assert_eq!(result.properties[0].name.offset, 13);
        assert_eq!(result.properties[0].name.line, 1);
        assert_eq!(result.properties[0].name.column, 4);
        assert_eq!(result.properties[0].value.offset, 22);
        assert_eq!(result.properties[0].value.line, 1);
        assert_eq!(result.properties[0].value.column, 13);
    }

    #[test]
    fn standalone_cr_position_tracking() {
        // standalone \r (old Mac) should also be treated as a newline
        let css = ":root {\r    --color: red;\r}";
        let result = test_parse(css);
        assert_eq!(result.properties[0].name.offset, 12);
        assert_eq!(result.properties[0].name.line, 1);
        assert_eq!(result.properties[0].name.column, 4);
        assert_eq!(result.properties[0].value.offset, 21);
        assert_eq!(result.properties[0].value.line, 1);
        assert_eq!(result.properties[0].value.column, 13);
    }

    // parse_style_attr tests

    #[test]
    fn inline_single_property() {
        let result = test_parse_style_attr("color: red;");
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].name.raw, "color");
        assert_eq!(result.properties[0].value.raw, "red");
    }

    #[test]
    fn inline_multiple_properties() {
        let result = test_parse_style_attr("color: red; font-size: 16px;");
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "color");
        assert_eq!(result.properties[0].value.raw, "red");
        assert_eq!(result.properties[1].name.raw, "font-size");
        assert_eq!(result.properties[1].value.raw, "16px");
    }

    #[test]
    fn inline_no_trailing_semicolon() {
        let result = test_parse_style_attr("color: red");
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw, "red");
    }

    #[test]
    fn inline_var_usage() {
        let result = test_parse_style_attr("color: var(--main-color);");
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw, "var(--main-color)");
    }

    #[test]
    fn inline_var_def() {
        let result = test_parse_style_attr("--color: red; --size: 16px;");
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "--color");
        assert_eq!(result.properties[0].value.raw, "red");
        assert_eq!(result.properties[1].name.raw, "--size");
        assert_eq!(result.properties[1].value.raw, "16px");
    }

    #[test]
    fn inline_empty() {
        let result = test_parse_style_attr("");
        assert_eq!(result.properties.len(), 0);
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
        assert_eq!(result.properties[0].name.raw, "content");
        assert_eq!(result.properties[0].value.raw, "\"a;b\"");
        assert_eq!(result.properties[1].name.raw, "color");
        assert_eq!(result.properties[1].value.raw, "red");
    }

    #[test]
    fn closing_brace_inside_string_value() {
        let css = ".a { content: \"}\"; color: red; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "content");
        assert_eq!(result.properties[0].value.raw, "\"}\"");
        assert_eq!(result.properties[1].name.raw, "color");
        assert_eq!(result.properties[1].value.raw, "red");
    }

    #[test]
    fn escaped_quote_in_string_value() {
        let css = r#".a { content: "he said \"hello\""; }"#;
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].name.raw, "content");
        assert_eq!(result.properties[0].value.raw, r#""he said \"hello\"""#);
    }

    #[test]
    fn unterminated_string_at_eof() {
        let css = ".a { content: \"hello";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].name.raw, "content");
        assert_eq!(result.properties[0].value.raw, "\"hello");
    }

    #[test]
    fn unterminated_comment() {
        let css = ".a { color: red; } /* never closed";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].name.raw, "color");
        assert_eq!(result.properties[0].value.raw, "red");
    }

    #[test]
    fn unterminated_comment_inside_value() {
        let css = ".a { color: red /* unclosed";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw, "red");
    }

    #[test]
    fn empty_value() {
        let css = ".a { --empty: ; --next: blue; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "--empty");
        assert_eq!(result.properties[0].value.raw, "");
        assert_eq!(result.properties[1].name.raw, "--next");
        assert_eq!(result.properties[1].value.raw, "blue");
    }

    #[test]
    fn no_spaces_around_colon() {
        let css = ".a{color:red;font-size:16px}";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "color");
        assert_eq!(result.properties[0].value.raw, "red");
        assert_eq!(result.properties[1].name.raw, "font-size");
        assert_eq!(result.properties[1].value.raw, "16px");
    }

    #[test]
    fn nested_blocks() {
        let css = ".a { .b { color: red; } font-size: 16px; }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 2);
        assert_eq!(result.properties[0].name.raw, "color");
        assert_eq!(result.properties[0].value.raw, "red");
        assert_eq!(result.properties[1].name.raw, "font-size");
        assert_eq!(result.properties[1].value.raw, "16px");
    }

    #[test]
    fn backslash_at_eof_in_string() {
        let css = ".a { content: \"test\\";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].name.raw, "content");
        assert_eq!(result.properties[0].value.raw, "\"test\\");
    }

    #[test]
    fn value_terminated_by_closing_brace() {
        let css = ".a { color: red }";
        let result = test_parse(css);
        assert_eq!(result.properties.len(), 1);
        assert_eq!(result.properties[0].value.raw, "red");
    }

    #[test]
    fn unterminated_comment_consumes_all_bytes() {
        let mut s = Scanner::new("/* {");
        s.skip_comment();
        assert!(s.is_eof(), "skip_comment should consume all bytes of an unterminated comment, but pos={} len={}", s.pos, s.bytes.len());
    }
}
