use std::error::Error;
use std::ops::Range;
use std::path::Path;

use lightningcss::properties::custom::{TokenList, TokenOrValue, Variable};
use lsp_server::{Message, Request, Response};
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Position};

use super::Server;
use super::description::{format_multi_def, format_single, resolve_to_raw_value};
use crate::owned_types::OwnedPropId;
use crate::parser::Property;
use crate::searcher::PropMapFor;
use crate::searcher::SearchResultFor;
use crate::searcher::conditions::variable_definitions::VariableDefinitions;
use crate::searcher::conditions::variable_usages::VariableUsages;
use crate::text_position::{byte_range_to_lsp_range, position_to_byte_offset};
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
        let rel_path = self.uri_to_rel_path(uri)?;

        let search_result = self.searcher.search();
        let usages = search_result.get_result_for(VariableUsages);
        let var_defs = search_result.get_prop_map_for::<VariableDefinitions>();
        let client_name = self.client_name.as_deref();

        compute_hover(source, &pos, &rel_path, &usages, &var_defs, client_name)
    }
}

fn compute_hover(
    source: &str,
    pos: &Position,
    file_path: &Path,
    usages: &SearchResultFor<'_, VariableUsages>,
    var_defs: &PropMapFor<'_, VariableDefinitions>,
    client_name: Option<&str>,
) -> Option<Hover> {
    let cursor = position_to_byte_offset(source, pos)?;

    let prop = usages
        .iter()
        .filter(|p| p.file_path.as_ref() == file_path)
        .find(|p| value_contains_offset(p, cursor))?;

    let token_list = prop.token_list();
    let (var, name_range) = find_var_at_cursor(prop, token_list.inner(), cursor)?;

    let value = format_var_hover(var, var_defs, client_name)?;

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: Some(byte_range_to_lsp_range(source, name_range)),
    })
}

fn value_contains_offset(prop: &Property, cursor: usize) -> bool {
    let start = prop.value.offset;
    let end = start + prop.value.raw.len();
    (start..=end).contains(&cursor)
}

fn find_var_at_cursor<'t>(
    prop: &Property,
    tokens: &'t TokenList<'t>,
    cursor: usize,
) -> Option<(&'t Variable<'t>, Range<usize>)> {
    let value_raw = prop.value.raw.as_str();
    let base = prop.value.offset;

    collect_vars(tokens)
        .into_iter()
        .scan(0usize, |search_from, var| {
            let ident = &*var.name.ident.0;
            let rel = value_raw[*search_from..].find(ident)?;
            let ident_abs = base + *search_from + rel;
            *search_from += rel + ident.len();
            Some((var, (ident_abs - 2)..(ident_abs + ident.len())))
        })
        .find(|(_, range)| range.contains(&cursor))
}

fn collect_vars<'t>(tokens: &'t TokenList<'t>) -> Vec<&'t Variable<'t>> {
    let mut vars: Vec<&'t Variable<'t>> = Vec::new();

    for token in &tokens.0 {
        match token {
            TokenOrValue::Var(var) => {
                vars.push(var);
                if let Some(fb) = &var.fallback {
                    vars.extend(collect_vars(fb));
                }
            }
            TokenOrValue::Function(func) => {
                vars.extend(collect_vars(&func.arguments));
            }
            _ => {}
        }
    }

    vars
}

fn format_var_hover(
    var: &Variable<'_>,
    var_defs: &PropMapFor<'_, VariableDefinitions>,
    client_name: Option<&str>,
) -> Option<String> {
    let prop_id = OwnedPropId::from(var.name.ident.0.to_string());
    match var_defs.get(&prop_id).as_deref() {
        Some(props) if props.len() > 1 => format_multi_def(props, var_defs, client_name, true),
        Some([prop]) => Some(format_single(
            &resolve_to_raw_value(prop, var_defs, 0)?,
            true,
        )),
        _ => {
            let vars = var_defs.vars_map();
            let synthetic = TokenList(vec![TokenOrValue::Var(var.clone())]);
            let resolved = resolve_variables(&synthetic, &vars).ok()?;
            Some(format_single(&resolved, true))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned_types::OwnedStr;
    use crate::parser;
    use crate::searcher::Searcher;
    use std::path::PathBuf;
    use std::rc::Rc;

    struct Fixture {
        searcher: Searcher,
        file_path: PathBuf,
    }

    impl Fixture {
        fn new(css: &str) -> Self {
            let file_path = PathBuf::from("test.css");
            let parse_result =
                parser::css::parse(&OwnedStr::from(css), &Rc::from(file_path.clone()));
            let mut searcher = Searcher::new()
                .add_condition(VariableDefinitions::default())
                .add_condition(VariableUsages);
            searcher.update_file(&parse_result.file_path, std::slice::from_ref(&parse_result));
            Self {
                searcher,
                file_path,
            }
        }

        fn hover(&self, source: &str, line: u32, character: u32) -> Option<Hover> {
            let result = self.searcher.search();
            let usages = result.get_result_for(VariableUsages);
            let map = result.get_prop_map_for::<VariableDefinitions>();
            compute_hover(
                source,
                &lsp_types::Position { line, character },
                &self.file_path,
                &usages,
                &map,
                None,
            )
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
