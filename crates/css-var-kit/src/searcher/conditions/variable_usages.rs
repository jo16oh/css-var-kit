use crate::parser::Property;
use crate::searcher::SearchCondition;

pub struct VariableUsages;

impl SearchCondition for VariableUsages {
    fn matches(&self, prop: &Property) -> bool {
        contains_variable_reference(prop.value.raw.as_ref())
    }
}

fn contains_variable_reference(value: &str) -> bool {
    let bytes = value.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        match bytes[i] {
            b'"' | b'\'' => i = end_of_string(bytes, i),
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => i = end_of_comment(bytes, i),
            b'u' | b'U' if at_token_boundary(bytes, i) && matches_func_open(bytes, i, b"url") => {
                i = end_of_url(bytes, i + 4);
            }
            b'v' | b'V' if at_token_boundary(bytes, i) && matches_func_open(bytes, i, b"var") => {
                return true;
            }
            b'-' if at_token_boundary(bytes, i)
                && i + 2 < len
                && bytes[i + 1] == b'-'
                && is_ident_continue(bytes[i + 2]) =>
            {
                return true;
            }
            _ => i += 1,
        }
    }
    false
}

fn at_token_boundary(bytes: &[u8], i: usize) -> bool {
    i == 0 || !is_ident_continue(bytes[i - 1])
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b >= 0x80
}

fn matches_func_open(bytes: &[u8], i: usize, name: &[u8]) -> bool {
    let end = i + name.len();
    end < bytes.len() && bytes[i..end].eq_ignore_ascii_case(name) && bytes[end] == b'('
}

fn end_of_string(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => i += 2,
            b if b == quote => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

fn end_of_comment(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 2;
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            return i + 2;
        }
        i += 1;
    }
    bytes.len()
}

fn end_of_url(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    let mut paren = 1i32;
    while i < bytes.len() && paren > 0 {
        match bytes[i] {
            b'"' | b'\'' => i = end_of_string(bytes, i),
            b'(' => {
                paren += 1;
                i += 1;
            }
            b')' => {
                paren -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned_types::OwnedStr;
    use crate::parser;
    use std::path::PathBuf;
    use std::rc::Rc;

    fn matches_value(value: &str) -> bool {
        let css = format!(".a {{ color: {}; }}", value);
        let result = parser::css::parse(&OwnedStr::from(css), &Rc::from(PathBuf::from("test.css")));
        let cond = VariableUsages;
        cond.matches(&result.properties[0])
    }

    #[test]
    fn matches_var_usage() {
        assert!(matches_value("var(--color)"));
        assert!(matches_value("var(--main-color)"));
        assert!(matches_value("1px solid var(--border-color)"));
    }

    #[test]
    fn matches_bare_dashed_ident() {
        assert!(matches_value("--my-animation"));
    }

    #[test]
    fn matches_nested_in_function() {
        assert!(matches_value("calc(var(--size) * 2)"));
    }

    #[test]
    fn rejects_no_var() {
        assert!(!matches_value("red"));
        assert!(!matches_value("16px"));
    }

    #[test]
    fn rejects_bare_double_dash() {
        assert!(!matches_value("--"));
    }

    #[test]
    fn rejects_dashed_ident_inside_double_quoted_string() {
        assert!(!matches_value("\"--not-a-var\""));
    }

    #[test]
    fn rejects_dashed_ident_inside_single_quoted_string() {
        assert!(!matches_value("'--not-a-var'"));
    }

    #[test]
    fn rejects_var_call_inside_string() {
        assert!(!matches_value("\"var(--x)\""));
    }

    #[test]
    fn rejects_dashed_ident_inside_quoted_url() {
        assert!(!matches_value("url(\"file--name.png\")"));
    }

    #[test]
    fn rejects_dashed_ident_inside_unquoted_url() {
        assert!(!matches_value("url(file--name.png)"));
    }

    #[test]
    fn rejects_var_call_inside_unquoted_url() {
        assert!(!matches_value("url(var(--x))"));
    }

    #[test]
    fn rejects_dashed_ident_glued_to_preceding_ident() {
        // `font--name` is a single identifier-like token, not a `--name` reference.
        assert!(!matches_value("font--name"));
    }

    #[test]
    fn matches_var_call_uppercase() {
        assert!(matches_value("VAR(--x)"));
        assert!(matches_value("Var(--x)"));
    }

    #[test]
    fn matches_real_var_alongside_string_with_dashes() {
        assert!(matches_value("\"--decoy\" var(--real)"));
    }

    #[test]
    fn rejects_dashed_ident_inside_block_comment() {
        // Comments don't normally survive into the value (parser strips them),
        // but be defensive.
        assert!(!matches_value("/* --decoy */ red"));
    }
}
