#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ResolveError {
    #[error("Variable not found: {0}")]
    VariableNotFound(String),
    #[error("Invalid syntax in variable declaration")]
    InvalidSyntax,
}

pub fn resolve_vars<'src>(
    value: &'src str,
    lookup: &impl Fn(&str) -> Option<&'src str>,
) -> Result<String, ResolveError> {
    let Some(start_idx) = value.find("var(") else {
        return Ok(value.to_string());
    };

    let prefix = &value[..start_idx];
    let after_open = &value[start_idx + 4..];

    let Some(close_idx) = find_closing_paren(after_open) else {
        return Ok(value.to_string());
    };

    let inner_content = &after_open[..close_idx];
    let suffix = &after_open[close_idx + 1..];

    let (name, fallback) = parse_var_contents(inner_content).ok_or(ResolveError::InvalidSyntax)?;

    let resolved_var = lookup(name)
        .or(fallback)
        .ok_or_else(|| ResolveError::VariableNotFound(name.to_string()))
        .and_then(|val| resolve_vars(val, lookup))?;

    let resolved_suffix = resolve_vars(suffix, lookup)?;

    Ok(format!("{prefix}{resolved_var}{resolved_suffix}"))
}

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
            resolve_vars("var(--color)", &lookup).unwrap(),
            "red".to_string(),
        );
    }

    #[test]
    fn var_in_mixed_value() {
        assert_eq!(
            resolve_vars("1px solid var(--color)", &lookup).unwrap(),
            "1px solid red".to_string(),
        );
    }

    #[test]
    fn multiple_vars() {
        assert_eq!(
            resolve_vars("var(--size) var(--color)", &lookup).unwrap(),
            "16px red".to_string(),
        );
    }

    #[test]
    fn incomplete_multiple_vars() {
        assert_eq!(
            resolve_vars("var(--size) var(--color", &lookup).unwrap(),
            "16px var(--color".to_string(),
        );
    }

    #[test]
    fn undefined_var_no_fallback() {
        assert_eq!(
            resolve_vars("var(--missing)", &lookup),
            Err(ResolveError::VariableNotFound("--missing".to_string()))
        );
    }

    #[test]
    fn undefined_var_with_fallback() {
        assert_eq!(
            resolve_vars("var(--missing, blue)", &lookup).unwrap(),
            "blue".to_string(),
        );
    }

    #[test]
    fn nested_var_in_fallback() {
        assert_eq!(
            resolve_vars("var(--missing, var(--color))", &lookup).unwrap(),
            "red".to_string(),
        );
    }

    #[test]
    fn deeply_nested_fallback() {
        assert_eq!(
            resolve_vars("var(--a, var(--b, var(--color)))", &lookup).unwrap(),
            "red".to_string(),
        );
    }

    #[test]
    fn resolved_value_contains_var() {
        // --nested resolves to "var(--color)", which should be further resolved
        assert_eq!(
            resolve_vars("var(--nested)", &lookup).unwrap(),
            "red".to_string(),
        );
    }

    #[test]
    fn no_var_passthrough() {
        assert_eq!(
            resolve_vars("1px solid red", &lookup).unwrap(),
            "1px solid red".to_string(),
        );
    }

    #[test]
    fn var_with_whitespace() {
        assert_eq!(
            resolve_vars("var(  --color  )", &lookup).unwrap(),
            "red".to_string(),
        );
    }

    #[test]
    fn var_with_complex_fallback() {
        assert_eq!(
            resolve_vars("var(--missing, 1px solid blue)", &lookup).unwrap(),
            "1px solid blue".to_string(),
        );
    }

    #[test]
    fn var_inside_function() {
        assert_eq!(
            resolve_vars("calc(var(--size) + 4px)", &lookup).unwrap(),
            "calc(16px + 4px)".to_string(),
        );
    }

    #[test]
    fn all_undefined_no_fallback() {
        assert_eq!(
            resolve_vars("var(--a) var(--b)", &lookup),
            Err(ResolveError::VariableNotFound("--a".to_string())),
        );
    }

    #[test]
    fn fallback_with_nested_parens() {
        assert_eq!(
            resolve_vars("var(--missing, calc(10px + 5px))", &lookup).unwrap(),
            "calc(10px + 5px)".to_string(),
        );
    }

    #[test]
    fn empty_value() {
        assert_eq!(resolve_vars("", &lookup).unwrap(), "".to_string(),);
    }

    #[test]
    fn fallback_with_string_containing_paren() {
        assert_eq!(
            resolve_vars("var(--missing, ')')", &lookup).unwrap(),
            "''".to_string(),
        );
    }
}
