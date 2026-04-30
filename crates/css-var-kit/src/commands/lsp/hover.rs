use std::error::Error;
use std::path::Path;

use lightningcss::properties::custom::{TokenList, TokenOrValue, Variable};
use lsp_server::{Message, Request, Response};
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Position, Range};

use super::Server;
use super::definition::extract_variable_at_cursor;
use super::description::{
    format_multi_def, format_single, multi_def_separator, resolve_to_raw_value,
};
use crate::owned_types::OwnedPropId;
use crate::parser::Property;
use crate::searcher::PropMapFor;
use crate::searcher::SearchResultFor;
use crate::searcher::conditions::variable_definitions::VariableDefinitions;
use crate::searcher::conditions::variable_usages::VariableUsages;
use crate::text_position::{byte_offset_to_utf16, position_to_byte_offset};
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
        let separator = multi_def_separator(self.client_name.as_deref());

        compute_hover(source, &pos, &rel_path, &usages, &var_defs, separator)
    }
}

fn compute_hover(
    source: &str,
    pos: &Position,
    file_path: &Path,
    usages: &SearchResultFor<'_, VariableUsages>,
    var_defs: &PropMapFor<'_, VariableDefinitions>,
    multi_def_separator: &str,
) -> Option<Hover> {
    let cursor = position_to_byte_offset(source, pos)?;
    let var_at_cursor = extract_variable_at_cursor(source, pos)?;

    let prop = usages
        .iter()
        .filter(|p| p.file_path.as_ref() == file_path)
        .find(|p| value_contains_offset(p, cursor))?;

    let token_list = prop.token_list();
    let var = find_var_by_name(token_list.inner(), &var_at_cursor.name)?;

    let value = format_var_hover(var, var_defs, multi_def_separator)?;

    let line_str = source.lines().nth(pos.line as usize)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: Some(Range {
            start: Position {
                line: pos.line,
                character: byte_offset_to_utf16(line_str, var_at_cursor.byte_start),
            },
            end: Position {
                line: pos.line,
                character: byte_offset_to_utf16(line_str, var_at_cursor.byte_end),
            },
        }),
    })
}

fn value_contains_offset(prop: &Property, cursor: usize) -> bool {
    let start = prop.value.offset;
    let end = start + prop.value.raw.len();
    (start..=end).contains(&cursor)
}

fn find_var_by_name<'t>(tokens: &'t TokenList<'t>, target: &str) -> Option<&'t Variable<'t>> {
    tokens.0.iter().find_map(|token| match token {
        TokenOrValue::Var(var) if &*var.name.ident.0 == target => Some(var),
        TokenOrValue::Var(var) => var
            .fallback
            .as_ref()
            .and_then(|fb| find_var_by_name(fb, target)),
        TokenOrValue::Function(func) => find_var_by_name(&func.arguments, target),
        _ => None,
    })
}

fn format_var_hover(
    var: &Variable<'_>,
    var_defs: &PropMapFor<'_, VariableDefinitions>,
    multi_def_separator: &str,
) -> Option<String> {
    let prop_id = OwnedPropId::from(var.name.ident.0.to_string());
    match var_defs.get(&prop_id).as_deref() {
        Some(props) if props.len() > 1 => {
            format_multi_def(props, var_defs, multi_def_separator, true)
        }
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
                "\n\n",
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
