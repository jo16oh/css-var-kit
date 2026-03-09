//! Resolves `var()` references in a CSS property value string.
//!
//! Given a raw CSS value and a lookup function for variable definitions,
//! replaces each `var(--name)` or `var(--name, fallback)` with the resolved value.
//! Handles nested `var()` in fallbacks and mixed values like `1px solid var(--color)`.

/// The result of resolving `var()` references in a value.
#[derive(Debug, PartialEq)]
pub enum ResolveResult {
    /// All `var()` references were resolved. Contains the fully resolved value string.
    Resolved(String),
    /// At least one `var()` could not be resolved (undefined variable with no fallback).
    Unresolved,
}

/// Resolves all `var()` references in a CSS value string.
///
/// The `lookup` function takes a variable name (e.g., `"--color"`) and returns
/// `Some(value)` if the variable is defined, or `None` if it is not.
///
/// When a variable is undefined, the fallback value is used if present.
/// If neither the variable nor a fallback is available, returns `ResolveResult::Unresolved`.
///
/// # Examples
///
/// ```ignore
/// // Simple resolution
/// resolve_vars("var(--color)", |name| match name { "--color" => Some("red"), _ => None })
/// // => Resolved("red")
///
/// // Mixed value
/// resolve_vars("1px solid var(--color)", |name| match name { "--color" => Some("red"), _ => None })
/// // => Resolved("1px solid red")
///
/// // Fallback
/// resolve_vars("var(--missing, blue)", |_| None)
/// // => Resolved("blue")
/// ```
pub fn resolve_vars<'src>(value: &'src str, lookup: impl Fn(&str) -> Option<&'src str>) -> ResolveResult {
    match resolve_inner(value, &lookup) {
        Some(result) => ResolveResult::Resolved(result),
        None => ResolveResult::Unresolved,
    }
}

/// Inner recursive resolver. Returns `None` if any var() could not be resolved.
fn resolve_inner<'src>(value: &'src str, lookup: &impl Fn(&str) -> Option<&'src str>) -> Option<String> {
    let Some(var_start) = value.find("var(") else {
        return Some(value.to_string());
    };

    let prefix = &value[..var_start];
    let inside = &value[var_start + 4..];
    let close = find_closing_paren(inside)?;
    let (name, fallback) = parse_var_contents(&inside[..close])?;

    let resolved_var = lookup(name)
        .or(fallback)
        .and_then(|val| resolve_inner(val, lookup))?;

    let rest = resolve_inner(&inside[close + 1..], lookup)?;

    Some(format!("{prefix}{resolved_var}{rest}"))
}

/// Parses the contents between `var(` and its matching `)`.
///
/// Returns `(name, fallback)` where fallback is the part after the first comma, if any.
fn parse_var_contents(contents: &str) -> Option<(&str, Option<&str>)> {
    let (name, fallback) = match contents.find(',') {
        Some(comma) => (contents[..comma].trim(), Some(contents[comma + 1..].trim())),
        None => (contents.trim(), None),
    };

    if !name.starts_with("--") {
        return None;
    }

    Some((name, fallback))
}

/// Finds the closing `)` for an already-opened `(`.
///
/// `input` is the content after the opening `(` (e.g., after `var(`).
/// Tracks nested parentheses and skips quoted strings.
/// Returns the byte offset of the closing `)` within `input`,
/// or `None` if no matching `)` is found.
fn find_closing_paren(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut pos = 0;
    let mut depth: u32 = 1;

    while pos < bytes.len() && depth > 0 {
        match bytes[pos] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            b'\'' | b'"' => {
                pos = skip_string(bytes, pos);
                continue;
            }
            _ => {}
        }
        pos += 1;
    }

    None
}

/// Skips over a quoted string (single or double), returning the position after the closing quote.
fn skip_string(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut pos = start + 1;
    while pos < bytes.len() {
        if bytes[pos] == b'\\' {
            pos += 2;
        } else if bytes[pos] == quote {
            return pos + 1;
        } else {
            pos += 1;
        }
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(name: &str) -> Option<&'static str> {
        match name {
            "--color" => Some("red"),
            "--size" => Some("16px"),
            "--spacing" => Some("8px"),
            "--nested" => Some("var(--color)"),
            _ => None,
        }
    }

    #[test]
    fn simple_var() {
        assert_eq!(
            resolve_vars("var(--color)", lookup),
            ResolveResult::Resolved("red".into()),
        );
    }

    #[test]
    fn var_in_mixed_value() {
        assert_eq!(
            resolve_vars("1px solid var(--color)", lookup),
            ResolveResult::Resolved("1px solid red".into()),
        );
    }

    #[test]
    fn multiple_vars() {
        assert_eq!(
            resolve_vars("var(--size) var(--color)", lookup),
            ResolveResult::Resolved("16px red".into()),
        );
    }

    #[test]
    fn undefined_var_no_fallback() {
        assert_eq!(
            resolve_vars("var(--missing)", lookup),
            ResolveResult::Unresolved
        );
    }

    #[test]
    fn undefined_var_with_fallback() {
        assert_eq!(
            resolve_vars("var(--missing, blue)", lookup),
            ResolveResult::Resolved("blue".into()),
        );
    }

    #[test]
    fn nested_var_in_fallback() {
        assert_eq!(
            resolve_vars("var(--missing, var(--color))", lookup),
            ResolveResult::Resolved("red".into()),
        );
    }

    #[test]
    fn deeply_nested_fallback() {
        assert_eq!(
            resolve_vars("var(--a, var(--b, var(--color)))", lookup),
            ResolveResult::Resolved("red".into()),
        );
    }

    #[test]
    fn resolved_value_contains_var() {
        // --nested resolves to "var(--color)", which should be further resolved
        assert_eq!(
            resolve_vars("var(--nested)", lookup),
            ResolveResult::Resolved("red".into()),
        );
    }

    #[test]
    fn no_var_passthrough() {
        assert_eq!(
            resolve_vars("1px solid red", lookup),
            ResolveResult::Resolved("1px solid red".into()),
        );
    }

    #[test]
    fn var_with_whitespace() {
        assert_eq!(
            resolve_vars("var(  --color  )", lookup),
            ResolveResult::Resolved("red".into()),
        );
    }

    #[test]
    fn var_with_complex_fallback() {
        assert_eq!(
            resolve_vars("var(--missing, 1px solid blue)", lookup),
            ResolveResult::Resolved("1px solid blue".into()),
        );
    }

    #[test]
    fn var_inside_function() {
        assert_eq!(
            resolve_vars("calc(var(--size) + 4px)", lookup),
            ResolveResult::Resolved("calc(16px + 4px)".into()),
        );
    }

    #[test]
    fn all_undefined_no_fallback() {
        assert_eq!(
            resolve_vars("var(--a) var(--b)", lookup),
            ResolveResult::Unresolved,
        );
    }

    #[test]
    fn fallback_with_nested_parens() {
        assert_eq!(
            resolve_vars("var(--missing, calc(10px + 5px))", lookup),
            ResolveResult::Resolved("calc(10px + 5px)".into()),
        );
    }

    #[test]
    fn empty_value() {
        assert_eq!(
            resolve_vars("", lookup),
            ResolveResult::Resolved("".into()),
        );
    }

    #[test]
    fn fallback_with_string_containing_paren() {
        assert_eq!(
            resolve_vars("var(--missing, ')')", lookup),
            ResolveResult::Resolved("')'".into()),
        );
    }
}
