use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use lightningcss::properties::custom::TokenOrValue;

use crate::color_value::parse_to_rgba;
use crate::owned_types::OwnedPropId;
use crate::parser::Property;
use crate::searcher::PropMapFor;
use crate::searcher::conditions::variable_definitions::VariableDefinitions;

pub fn multi_def_separator(client_name: Option<&str>) -> &'static str {
    match client_name {
        Some(name) if name.eq_ignore_ascii_case("helix") => "  \n",
        _ => "\n\n",
    }
}

pub fn format_single(resolved: &str, include_swatch: bool) -> String {
    match parse_to_rgba(resolved).filter(|_| include_swatch) {
        Some(color) => format!("{} `{resolved}`", swatch_markdown(&color)),
        None => format!("`{resolved}`"),
    }
}

pub fn format_multi_def(
    props: &[&Property],
    var_defs: &PropMapFor<'_, VariableDefinitions>,
    separator: &str,
    include_swatch: bool,
) -> Option<String> {
    let lines: Vec<String> = props
        .iter()
        .filter_map(|prop| {
            let resolved = resolve_to_raw_value(prop, var_defs, 0)?;
            let location = format!("{}:{}", prop.file_path.display(), prop.ident.line + 1);
            Some(match parse_to_rgba(&resolved).filter(|_| include_swatch) {
                Some(color) => format!("{} `{resolved}` — {location}", swatch_markdown(&color)),
                None => format!("`{resolved}` — {location}"),
            })
        })
        .collect();

    (!lines.is_empty()).then(|| lines.join(separator))
}

pub fn resolve_to_raw_value(
    prop: &Property,
    var_defs: &PropMapFor<'_, VariableDefinitions>,
    depth: usize,
) -> Option<String> {
    if depth > 32 {
        return None;
    }
    let token_list = prop.token_list().inner();
    if let [TokenOrValue::Var(var)] = token_list.0.as_slice() {
        if var.fallback.is_none() {
            let prop_id = OwnedPropId::from(var.name.ident.0.to_string());
            if let Some(next) = var_defs.get(&prop_id).and_then(|defs| defs.last().copied()) {
                return resolve_to_raw_value(next, var_defs, depth + 1);
            }
        }
    }
    Some(prop.value.raw.as_str().to_string())
}

fn swatch_markdown(color: &lsp_types::Color) -> String {
    let r = (color.red * 255.0).round() as u8;
    let g = (color.green * 255.0).round() as u8;
    let b = (color.blue * 255.0).round() as u8;
    let a = color.alpha;
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='14' height='14'>\
         <rect width='14' height='14' fill='rgba({r},{g},{b},{a})' \
         stroke='rgba(0,0,0,0.2)' stroke-width='0.5'/></svg>"
    );
    let encoded = BASE64.encode(svg.as_bytes());
    format!("![](data:image/svg+xml;base64,{encoded})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separator_picks_hard_break_for_helix() {
        assert_eq!(multi_def_separator(Some("helix")), "  \n");
        assert_eq!(multi_def_separator(Some("Helix")), "  \n");
    }

    #[test]
    fn separator_defaults_to_paragraph_break() {
        assert_eq!(multi_def_separator(None), "\n\n");
        assert_eq!(multi_def_separator(Some("Visual Studio Code")), "\n\n");
        assert_eq!(multi_def_separator(Some("Zed")), "\n\n");
    }
}
