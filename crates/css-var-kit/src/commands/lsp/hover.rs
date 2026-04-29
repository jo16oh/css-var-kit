use std::error::Error;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use lightningcss::properties::custom::TokenOrValue;
use lsp_server::{Message, Request, Response};
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Position, Range};

use super::Server;
use super::definition::{VariableAtCursor, extract_variable_at_cursor};
use crate::color_value::parse_to_rgba;
use crate::owned_types::{OwnedPropId, OwnedStr};
use crate::parser::Property;
use crate::searcher::PropMapFor;
use crate::searcher::conditions::variable_definitions::{VariableDefinitions, VarsMap};
use crate::text_position::byte_offset_to_utf16;
use crate::variable_resolver::resolve_variables;

impl Server<'_> {
    pub fn handle_hover_request(&self, req: Request) -> Result<(), Box<dyn Error>> {
        let params: HoverParams = serde_json::from_value(req.params)?;
        self.log(&format!(
            "textDocument/hover: {}",
            params
                .text_document_position_params
                .text_document
                .uri
                .as_str()
        ));
        let result = self.handle_hover(&params);
        let response = Response::new_ok(req.id, result);
        self.connection.sender.send(Message::Response(response))?;
        Ok(())
    }

    fn handle_hover(&self, params: &HoverParams) -> Option<Hover> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let source = self.opened_documents.get(uri)?;

        let search_result = self.searcher.search();
        let var_defs = search_result.get_prop_map_for::<VariableDefinitions>();

        compute_hover(source, &pos, &var_defs)
    }
}

fn compute_hover(
    source: &str,
    pos: &Position,
    var_defs: &PropMapFor<'_, VariableDefinitions>,
) -> Option<Hover> {
    let var = extract_variable_at_cursor(source, pos)?;
    let line_str = source.lines().nth(pos.line as usize)?;
    let var_call = extract_enclosing_var_call(line_str, var.byte_start)?;

    let prop_id = OwnedPropId::from(var.name.clone());
    let defs = var_defs.get(&prop_id);

    let value = match defs.as_deref() {
        Some(props) if props.len() > 1 => format_multi_def(props, var_defs)?,
        Some([prop]) => format_single(&resolve_to_raw_value(prop, var_defs, 0)?),
        _ => {
            let vars = var_defs.vars_map();
            let resolved = resolve_var_call(var_call, &vars)?;
            format_single(&resolved)
        }
    };

    let range = make_range(line_str, pos.line, &var);
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: Some(range),
    })
}

fn extract_enclosing_var_call(line: &str, name_start: usize) -> Option<&str> {
    let bytes = line.as_bytes();
    let scan_end = name_start.min(bytes.len());

    let mut open_paren = None;
    for i in (0..scan_end).rev() {
        match bytes[i] {
            b'(' => {
                open_paren = Some(i);
                break;
            }
            b';' | b'{' | b'}' => return None,
            _ => {}
        }
    }
    let open_paren = open_paren?;

    let prefix_end = line[..open_paren]
        .trim_end_matches(|c: char| c.is_ascii_whitespace())
        .len();
    if !line[..prefix_end].ends_with("var") {
        return None;
    }
    let var_start = prefix_end - 3;

    let mut depth = 1usize;
    let mut close_paren = None;
    for (k, &b) in bytes.iter().enumerate().skip(open_paren + 1) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    close_paren = Some(k);
                    break;
                }
            }
            _ => {}
        }
    }
    Some(&line[var_start..=close_paren?])
}

fn resolve_var_call(var_call: &str, vars: &VarsMap<'_>) -> Option<String> {
    let owned = OwnedStr::from(var_call);
    let parsed = crate::owned_types::OwnedTokenList::parse(&owned).ok()?;
    resolve_variables(parsed.inner(), vars).ok()
}

fn format_single(resolved: &str) -> String {
    match parse_to_rgba(resolved) {
        Some(color) => format!("{} `{resolved}`", swatch_markdown(&color)),
        None => format!("`{resolved}`"),
    }
}

fn format_multi_def(
    props: &[&Property],
    var_defs: &PropMapFor<'_, VariableDefinitions>,
) -> Option<String> {
    let lines: Vec<String> = props
        .iter()
        .filter_map(|prop| {
            let resolved = resolve_to_raw_value(prop, var_defs, 0)?;
            let location = format!("{}:{}", prop.file_path.display(), prop.ident.line + 1);
            Some(match parse_to_rgba(&resolved) {
                Some(color) => format!("{} `{resolved}` — {location}", swatch_markdown(&color)),
                None => format!("`{resolved}` — {location}"),
            })
        })
        .collect();

    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn resolve_to_raw_value(
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

fn make_range(line_str: &str, line: u32, var: &VariableAtCursor) -> Range {
    let start_char = byte_offset_to_utf16(line_str, var.byte_start);
    let end_char = byte_offset_to_utf16(line_str, var.byte_end);
    Range {
        start: Position {
            line,
            character: start_char,
        },
        end: Position {
            line,
            character: end_char,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned_types::OwnedStr;
    use crate::parser;
    use crate::searcher::Searcher;
    use crate::searcher::conditions::variable_definitions::VariableDefinitions;
    use std::path::PathBuf;
    use std::rc::Rc;

    struct Fixture {
        searcher: Searcher,
    }

    impl Fixture {
        fn new(css: &str) -> Self {
            let parse_result =
                parser::css::parse(&OwnedStr::from(css), &Rc::from(PathBuf::from("test.css")));
            let mut searcher = Searcher::new().add_condition(VariableDefinitions::default());
            searcher.update_file(&parse_result.file_path, std::slice::from_ref(&parse_result));
            Self { searcher }
        }

        fn hover(&self, source: &str, line: u32, character: u32) -> Option<Hover> {
            let result = self.searcher.search();
            let map = result.get_prop_map_for::<VariableDefinitions>();
            compute_hover(source, &Position { line, character }, &map)
        }
    }

    fn hover_text(h: &Hover) -> &str {
        match &h.contents {
            HoverContents::Markup(m) => &m.value,
            _ => panic!("expected markup contents"),
        }
    }

    #[test]
    fn hover_on_color_var_includes_swatch_and_value() {
        let css = ":root { --brand: #ff0000; }\n.x { color: var(--brand); }";
        let fixture = Fixture::new(css);
        let hover = fixture.hover(css, 1, 18).expect("hover present");
        let text = hover_text(&hover);
        assert!(
            text.contains("data:image/svg+xml;base64,"),
            "swatch missing: {text}"
        );
        assert!(
            text.contains("`#ff0000`"),
            "expected resolved value: {text}"
        );
    }

    #[test]
    fn hover_preserves_hsl_definition() {
        let css = ":root { --brand: hsl(0, 100%, 50%); }\n.x { color: var(--brand); }";
        let fixture = Fixture::new(css);
        let hover = fixture.hover(css, 1, 18).expect("hover present");
        let text = hover_text(&hover);
        assert!(
            text.contains("`hsl(0, 100%, 50%)`"),
            "expected raw hsl preserved: {text}"
        );
        assert!(
            text.contains("data:image/svg+xml;base64,"),
            "swatch missing: {text}"
        );
    }

    #[test]
    fn hover_preserves_hwb_definition() {
        let css = ":root { --brand: hwb(0 0% 0%); }\n.x { color: var(--brand); }";
        let fixture = Fixture::new(css);
        let hover = fixture.hover(css, 1, 18).expect("hover present");
        let text = hover_text(&hover);
        assert!(
            text.contains("`hwb(0 0% 0%)`"),
            "expected raw hwb preserved: {text}"
        );
    }

    #[test]
    fn hover_uses_empty_alt_text_for_swatch() {
        let css = ":root { --brand: #ff0000; }\n.x { color: var(--brand); }";
        let fixture = Fixture::new(css);
        let hover = fixture.hover(css, 1, 18).expect("hover present");
        let text = hover_text(&hover);
        assert!(text.contains("![]("), "expected empty alt text: {text}");
    }

    #[test]
    fn hover_on_non_color_var_shows_value_only() {
        let css = ":root { --size: 16px; }\n.x { padding: var(--size); }";
        let fixture = Fixture::new(css);
        let hover = fixture.hover(css, 1, 20).expect("hover present");
        let text = hover_text(&hover);
        assert!(text.contains("16px"), "value missing: {text}");
        assert!(
            !text.contains("data:image/svg"),
            "no swatch expected: {text}"
        );
    }

    #[test]
    fn hover_on_undefined_var_returns_none() {
        let css = ".x { color: var(--missing); }";
        let fixture = Fixture::new(css);
        assert!(fixture.hover(css, 0, 18).is_none());
    }

    #[test]
    fn hover_resolves_chained_vars() {
        let css = ":root { --a: var(--b); --b: #00ff00; }\n.x { color: var(--a); }";
        let fixture = Fixture::new(css);
        let hover = fixture.hover(css, 1, 18).expect("hover present");
        let text = hover_text(&hover);
        assert!(
            text.contains("data:image/svg+xml;base64,"),
            "swatch missing: {text}"
        );
        assert!(
            text.contains("`#00ff00`"),
            "expected resolved value: {text}"
        );
    }

    #[test]
    fn hover_on_definition_site_returns_none() {
        let css = ":root { --brand: #ff0000; }";
        let fixture = Fixture::new(css);
        assert!(fixture.hover(css, 0, 10).is_none());
    }

    #[test]
    fn hover_uses_fallback_when_var_undefined() {
        let css = ".x { color: var(--missing, red); }";
        let fixture = Fixture::new(css);
        let hover = fixture.hover(css, 0, 18).expect("hover present");
        let text = hover_text(&hover);
        assert!(text.contains("red"), "fallback value missing: {text}");
    }

    #[test]
    fn hover_lists_multiple_definitions() {
        let css =
            ":root { --brand: #ff0000; }\n.dark { --brand: #0000ff; }\n.x { color: var(--brand); }";
        let fixture = Fixture::new(css);
        let hover = fixture.hover(css, 2, 18).expect("hover present");
        let text = hover_text(&hover);
        let entry_count = text.lines().filter(|l| !l.is_empty()).count();
        assert_eq!(entry_count, 2, "expected 2 entries: {text}");
        assert!(
            text.contains("test.css:1"),
            "missing first def location: {text}"
        );
        assert!(
            text.contains("test.css:2"),
            "missing second def location: {text}"
        );
        let swatch_count = text.matches("data:image/svg+xml;base64,").count();
        assert_eq!(swatch_count, 2, "expected 2 swatches: {text}");
    }
}
