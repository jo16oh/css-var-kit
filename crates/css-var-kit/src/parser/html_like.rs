use std::{path::Path, rc::Rc};

use crate::owned::OwnedStr;

use super::css::{self, ParseResult};

struct StyleBlock {
    content: OwnedStr,
    line_offset: u32,
    // Column offset of the first content line only; resets to 0 after the first newline.
    column_offset: u32,
    byte_offset: usize,
}

fn is_lang_supported(lang: &str) -> bool {
    lang.eq_ignore_ascii_case("css") || lang.eq_ignore_ascii_case("scss")
}

/// Extracts `<style>` blocks from an HTML-like source.
/// Blocks with `lang="less"`, `lang="stylus"`, etc. are skipped;
/// no `lang`, `lang="css"`, and `lang="scss"` are accepted.
fn extract_style_blocks(source: OwnedStr) -> Vec<StyleBlock> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut pos = 0;
    let mut line = 0u32;
    let mut col = 0u32;
    let mut blocks = Vec::new();

    macro_rules! advance {
        ($n:expr) => {
            for _ in 0..$n {
                if pos < len {
                    match bytes[pos] {
                        b'\n' => {
                            line += 1;
                            col = 0;
                            pos += 1;
                        }
                        b'\r' => {
                            line += 1;
                            col = 0;
                            pos += 1;
                            // Treat CRLF as a single newline.
                            if pos < len && bytes[pos] == b'\n' {
                                pos += 1;
                            }
                        }
                        _ => {
                            col += 1;
                            pos += 1;
                        }
                    }
                }
            }
        };
    }

    while pos < len {
        if pos + 6 <= len && &bytes[pos..pos + 6] == b"<style" {
            let tag_start = pos;
            let tag_start_line = line;
            let tag_start_col = col;
            advance!(6);

            // Ignore `<styles>`, `<stylesheet>`, etc.
            if pos < len && is_ident_char(bytes[pos]) {
                continue;
            }

            let mut lang: Option<String> = None;

            loop {
                if pos >= len {
                    break;
                }
                match bytes[pos] {
                    // Self-closing `<style />` is not valid HTML but handle defensively.
                    b'/' => {
                        advance!(1);
                        if pos < len && bytes[pos] == b'>' {
                            advance!(1);
                        }
                        break;
                    }
                    b'>' => {
                        advance!(1);
                        break;
                    }
                    b' ' | b'\t' | b'\n' | b'\r' => {
                        advance!(1);
                    }
                    _ => {
                        let attr_name = scan_attr_name(bytes, &mut pos, &mut line, &mut col);
                        skip_ascii_whitespace(bytes, &mut pos, &mut line, &mut col);
                        if pos < len && bytes[pos] == b'=' {
                            advance!(1);
                            skip_ascii_whitespace(bytes, &mut pos, &mut line, &mut col);
                            let attr_value = scan_attr_value(bytes, &mut pos, &mut line, &mut col);
                            if attr_name.eq_ignore_ascii_case("lang") {
                                lang = Some(attr_value);
                            }
                        }
                    }
                }
            }

            let supported = match &lang {
                None => true, // no lang attr defaults to CSS
                Some(v) => is_lang_supported(v),
            };

            if !supported {
                continue;
            }

            let content_start = pos;
            let content_line = line;
            let content_col = col;

            let end = find_close_style_tag(bytes, pos);
            if end > content_start {
                blocks.push(StyleBlock {
                    content: source.slice(content_start..end),
                    line_offset: content_line,
                    column_offset: content_col,
                    byte_offset: content_start,
                });
            }

            // Advance past `</style>` for the next iteration.
            let close_len = end + "</style>".len();
            let skip = close_len.saturating_sub(pos).min(len - pos);
            advance!(skip);

            let _ = (tag_start, tag_start_line, tag_start_col); // suppress unused warnings
        } else {
            advance!(1);
        }
    }

    blocks
}

/// Returns the start position of `</style>`. Returns `from` if not found.
fn find_close_style_tag(bytes: &[u8], from: usize) -> usize {
    let needle = b"</style>";
    let len = bytes.len();
    let mut pos = from;
    while pos + needle.len() <= len {
        if bytes[pos..pos + needle.len()].eq_ignore_ascii_case(needle) {
            return pos;
        }
        pos += 1;
    }
    from
}

fn scan_attr_name<'b>(bytes: &'b [u8], pos: &mut usize, _line: &mut u32, col: &mut u32) -> &'b str {
    let start = *pos;
    while *pos < bytes.len() {
        match bytes[*pos] {
            b' ' | b'\t' | b'\n' | b'\r' | b'=' | b'>' | b'/' => break,
            _ => {
                *col += 1;
                *pos += 1;
            }
        }
    }
    // SAFETY: HTML-like files are assumed to be valid UTF-8.
    std::str::from_utf8(&bytes[start..*pos]).unwrap_or("")
}

fn scan_attr_value(bytes: &[u8], pos: &mut usize, line: &mut u32, col: &mut u32) -> String {
    if *pos >= bytes.len() {
        return String::new();
    }
    let quote = bytes[*pos];
    if quote == b'"' || quote == b'\'' {
        *col += 1;
        *pos += 1;
        let start = *pos;
        while *pos < bytes.len() && bytes[*pos] != quote {
            if bytes[*pos] == b'\n' {
                *line += 1;
                *col = 0;
            } else {
                *col += 1;
            }
            *pos += 1;
        }
        let value = std::str::from_utf8(&bytes[start..*pos])
            .unwrap_or("")
            .to_owned();
        if *pos < bytes.len() {
            *col += 1;
            *pos += 1;
        }
        value
    } else {
        let start = *pos;
        while *pos < bytes.len() {
            match bytes[*pos] {
                b' ' | b'\t' | b'\n' | b'\r' | b'>' => break,
                _ => {
                    *col += 1;
                    *pos += 1;
                }
            }
        }
        std::str::from_utf8(&bytes[start..*pos])
            .unwrap_or("")
            .to_owned()
    }
}

fn skip_ascii_whitespace(bytes: &[u8], pos: &mut usize, line: &mut u32, col: &mut u32) {
    while *pos < bytes.len() {
        match bytes[*pos] {
            b' ' | b'\t' => {
                *col += 1;
                *pos += 1;
            }
            b'\n' => {
                *line += 1;
                *col = 0;
                *pos += 1;
            }
            b'\r' => {
                *line += 1;
                *col = 0;
                *pos += 1;
                // Treat CRLF as a single newline.
                if *pos < bytes.len() && bytes[*pos] == b'\n' {
                    *pos += 1;
                }
            }
            _ => break,
        }
    }
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// Parses an HTML-like file (Vue, Svelte, Astro, etc.).
///
/// Extracts all `<style>` blocks and passes each to the CSS parser.
/// `Property.source` is set to the full file source, and `line`/`column`
/// are absolute positions within the file.
pub fn parse_html_like(source: &OwnedStr, file_path: &Rc<Path>) -> Vec<ParseResult> {
    extract_style_blocks(source.clone())
        .into_iter()
        .map(move |block| {
            css::parse_with_offset(
                &block.content,
                file_path,
                source,
                block.line_offset,
                block.column_offset,
                block.byte_offset,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn path() -> Rc<Path> {
        Rc::from(PathBuf::from("test.vue"))
    }

    #[test]
    fn extracts_simple_style_block() {
        let source = OwnedStr::from("<style>\n.a { --color: red; }\n</style>");
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].content.trim(), ".a { --color: red; }");
        // After consuming `>`, position is still on line 0; the `\n` is the first content char.
        assert_eq!(blocks[0].line_offset, 0);
        assert_eq!(blocks[0].column_offset, 7); // len("<style>") == 7
    }

    #[test]
    fn single_line_style_block_has_column_offset() {
        let source = OwnedStr::from("<style>.a { --color: red; }</style>");
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].content.as_str(), ".a { --color: red; }");
        assert_eq!(blocks[0].line_offset, 0);
        assert_eq!(blocks[0].column_offset, 7); // len("<style>") == 7
    }

    #[test]
    fn column_offset_with_preceding_html() {
        let source = OwnedStr::from("<p>x</p><style>.a { --x: 1; }</style>");
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
        // `<p>x</p>` (8) + `<style>` (7) = 15 chars consumed before content start
        assert_eq!(blocks[0].column_offset, 15);
    }

    #[test]
    fn skips_less_lang() {
        let source = OwnedStr::from(r#"<style lang="less">.a { @x: 1; }</style>"#);
        let blocks = extract_style_blocks(source);
        assert!(blocks.is_empty());
    }

    #[test]
    fn skips_stylus_lang() {
        let source = OwnedStr::from(r#"<style lang="stylus">.a\n  color red\n</style>"#);
        let blocks = extract_style_blocks(source);
        assert!(blocks.is_empty());
    }

    #[test]
    fn allows_css_lang() {
        let source = OwnedStr::from(r#"<style lang="css">.a { --x: 1px; }</style>"#);
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn allows_scss_lang() {
        let source = OwnedStr::from(r#"<style lang="scss">.a { --x: 1px; }</style>"#);
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn allows_no_lang() {
        let source = OwnedStr::from("<style>.a { --x: 1px; }</style>");
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn extracts_multiple_blocks() {
        let source =
            OwnedStr::from("<style>.a { --x: 1px; }</style>\n<style>.b { --y: 2px; }</style>");
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn ignores_style_tag_prefix_like_styles() {
        let source = OwnedStr::from("<styles>.a { --x: 1px; }</styles>");
        let blocks = extract_style_blocks(source);
        assert!(blocks.is_empty());
    }

    #[test]
    fn property_line_is_absolute_multiline() {
        // line 0: <template>...</template>
        // line 1: <style>
        // line 2: .a { --color: red; }
        // line 3: </style>
        let source =
            OwnedStr::from("<template></template>\n<style>\n.a { --color: red; }\n</style>");
        let results = parse_html_like(&source, &path());
        assert_eq!(results.len(), 1);
        let props = &results[0].properties;
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].ident.line, 2);
    }

    #[test]
    fn property_column_is_absolute_single_line() {
        // `--color` starts at column 12: `<style>` (7) + `.a { ` (5) = 12
        let source = OwnedStr::from("<style>.a { --color: red; }</style>");
        let results = parse_html_like(&source, &path());
        assert_eq!(results.len(), 1);
        let props = &results[0].properties;
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].ident.line, 0);
        assert_eq!(props[0].ident.column, 12); // 7 + 5 = 12
    }

    #[test]
    fn source_is_full_html_file() {
        let source = OwnedStr::from("<style>.a { --x: 1px; }</style>");
        let results = parse_html_like(&source, &path());
        assert_eq!(results[0].properties[0].source, source);
    }

    // --- Edge case tests ---

    #[test]
    fn crlf_line_endings_counted_as_single_newline() {
        // line 0: <template></template>
        // line 1: <style>
        // line 2: .a { --color: red; }
        // line 3: </style>
        let source =
            OwnedStr::from("<template></template>\r\n<style>\r\n.a { --color: red; }\r\n</style>");
        let results = parse_html_like(&source, &path());
        assert_eq!(results.len(), 1);
        let props = &results[0].properties;
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].ident.line, 2);
    }

    #[test]
    fn crlf_block_offset_is_correct() {
        // The content_line of the style block itself should reflect correct CRLF counting.
        let source = OwnedStr::from("<p></p>\r\n<style>\r\n.a { --x: 1; }\r\n</style>");
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
        // "<p></p>\r\n" — CRLF counts as one newline, so <style> is on line 1.
        // After consuming `<style>` and its closing `>`, content_line = 1.
        // (The `\r\n` after `>` is part of the content, not yet consumed.)
        assert_eq!(blocks[0].line_offset, 1);
    }

    #[test]
    fn lang_uppercase_css_is_accepted() {
        let source = OwnedStr::from(r#"<style lang="CSS">.a { --x: 1px; }</style>"#);
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn lang_uppercase_scss_is_accepted() {
        let source = OwnedStr::from(r#"<style lang="SCSS">.a { --x: 1px; }</style>"#);
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn lang_mixed_case_is_accepted() {
        let source = OwnedStr::from(r#"<style lang="Css">.a { --x: 1px; }</style>"#);
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn lang_single_quote_is_accepted() {
        let source = OwnedStr::from("<style lang='css'>.a { --x: 1px; }</style>");
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn lang_attr_after_other_attrs() {
        let source = OwnedStr::from(r#"<style scoped lang="css">.a { --x: 1px; }</style>"#);
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn lang_attr_before_other_attrs() {
        let source = OwnedStr::from(r#"<style lang="css" scoped>.a { --x: 1px; }</style>"#);
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn self_closing_style_tag_produces_no_block() {
        let source = OwnedStr::from("<style/>");
        let blocks = extract_style_blocks(source);
        assert!(blocks.is_empty());
    }

    #[test]
    fn unclosed_style_tag_produces_no_block() {
        let source = OwnedStr::from("<style>.a { --x: 1; }");
        let blocks = extract_style_blocks(source);
        assert!(blocks.is_empty());
    }

    #[test]
    fn closing_tag_is_case_insensitive() {
        let source = OwnedStr::from("<style>.a { --x: 1px; }</STYLE>");
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].content.as_str(), ".a { --x: 1px; }");
    }

    #[test]
    fn lang_attr_with_spaces_around_equals() {
        let source = OwnedStr::from("<style lang = \"css\">.a { --x: 1px; }</style>");
        let blocks = extract_style_blocks(source);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn empty_style_block_produces_no_block() {
        let source = OwnedStr::from("<style></style>");
        let blocks = extract_style_blocks(source);
        assert!(blocks.is_empty());
    }
}
