use std::error::Error;

use lightningcss::properties::custom::{TokenList, TokenOrValue};
use lsp_server::{Message, Request, Response};
use lsp_types::{
    ColorInformation, ColorPresentation, ColorPresentationParams, DocumentColorParams, Position,
    Range,
};

use super::Server;
use super::rename::find_var_in_source;
use crate::color_value;
use crate::parser::Property;
use crate::searcher::conditions::variable_definitions::{VariableDefinitions, VarsMap};
use crate::text_position::byte_col_to_utf16_in_source;
use crate::type_checker::value_kind::{ValueKind, ValueKindSet, kind_of};
use crate::variable_resolver::resolve_variables;

impl Server<'_> {
    pub fn handle_document_color_request(&self, req: Request) -> Result<(), Box<dyn Error>> {
        let params: DocumentColorParams = serde_json::from_value(req.params)?;
        self.log(&format!(
            "textDocument/documentColor: {}",
            params.text_document.uri.as_str()
        ));
        let result = self.handle_document_color(&params).unwrap_or_default();
        let response = Response::new_ok(req.id, result);
        self.connection.sender.send(Message::Response(response))?;
        Ok(())
    }

    pub fn handle_color_presentation_request(&self, req: Request) -> Result<(), Box<dyn Error>> {
        let _params: ColorPresentationParams = serde_json::from_value(req.params)?;
        let result: Vec<ColorPresentation> = Vec::new();
        let response = Response::new_ok(req.id, result);
        self.connection.sender.send(Message::Response(response))?;
        Ok(())
    }

    fn handle_document_color(&self, params: &DocumentColorParams) -> Option<Vec<ColorInformation>> {
        let rel_path = self.uri_to_rel_path(&params.text_document.uri)?;
        let parse_results = self.parse_cache.get(rel_path.as_path())?;

        let search_result = self.searcher.search();
        let var_defs = search_result.get_prop_map_for::<VariableDefinitions>();
        let vars = var_defs.vars_map();

        let infos: Vec<ColorInformation> = parse_results
            .iter()
            .flat_map(|pr| pr.properties.iter())
            .filter(|prop| !prop.ident.raw.starts_with("--"))
            .flat_map(|prop| collect_color_infos(prop, &vars))
            .collect();

        Some(infos)
    }
}

fn collect_color_infos(prop: &Property, vars: &VarsMap<'_>) -> Vec<ColorInformation> {
    let mut infos = Vec::new();
    let mut search_from = 0usize;
    walk_tokens(
        prop.token_list().inner(),
        prop,
        vars,
        &mut search_from,
        &mut infos,
    );
    infos
}

fn walk_tokens(
    token_list: &TokenList<'_>,
    prop: &Property,
    vars: &VarsMap<'_>,
    search_from: &mut usize,
    infos: &mut Vec<ColorInformation>,
) {
    for token in &token_list.0 {
        match token {
            TokenOrValue::Var(var) => {
                let name = &*var.name.ident.0;
                let position =
                    find_var_in_source(&prop.source, prop.value.offset, *search_from, name);
                if let Some((line, byte_col, byte_len, next_sf)) = position {
                    *search_from = next_sf;
                    if let Some(color) = resolve_var_color(token.clone(), vars) {
                        let range = make_range(&prop.source, line, byte_col, byte_len);
                        infos.push(ColorInformation { range, color });
                    }
                }
                if let Some(fallback) = &var.fallback {
                    walk_tokens(fallback, prop, vars, search_from, infos);
                }
            }
            TokenOrValue::Function(func) => {
                walk_tokens(&func.arguments, prop, vars, search_from, infos);
            }
            _ => {}
        }
    }
}

fn resolve_var_color(token: TokenOrValue<'_>, vars: &VarsMap<'_>) -> Option<lsp_types::Color> {
    let single = TokenList(vec![token]);
    let resolved = resolve_variables(&single, vars).ok()?;
    let kind = kind_of(&resolved);
    if !matches!(kind, ValueKind::Single(set) if set.intersects(ValueKindSet::COLOR)) {
        return None;
    }
    color_value::parse_to_rgba(&resolved)
}

fn make_range(source: &str, line: u32, byte_col: u32, byte_len: u32) -> Range {
    let start_char = byte_col_to_utf16_in_source(source, line, byte_col);
    let end_char = byte_col_to_utf16_in_source(source, line, byte_col + byte_len);
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
    use std::path::PathBuf;
    use std::rc::Rc;

    use super::*;
    use crate::owned_types::OwnedStr;
    use crate::parser;
    use crate::searcher::Searcher;
    use crate::searcher::conditions::variable_definitions::VariableDefinitions;

    use lsp_types::Color;

    fn collect_for(css: &str) -> Vec<ColorInformation> {
        let parse_result =
            parser::css::parse(&OwnedStr::from(css), &Rc::from(PathBuf::from("test.css")));
        let mut searcher = Searcher::new().add_condition(VariableDefinitions::default());
        searcher.update_file(&parse_result.file_path, std::slice::from_ref(&parse_result));
        let result = searcher.search();
        let var_defs = result.get_prop_map_for::<VariableDefinitions>();
        let vars = var_defs.vars_map();

        parse_result
            .properties
            .iter()
            .filter(|p| !p.ident.raw.starts_with("--"))
            .flat_map(|p| collect_color_infos(p, &vars))
            .collect()
    }

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1.0 / 255.0 + f32::EPSILON
    }

    fn red() -> Color {
        Color {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        }
    }

    fn assert_color(actual: Color, expected: Color) {
        assert!(
            approx_eq(actual.red, expected.red)
                && approx_eq(actual.green, expected.green)
                && approx_eq(actual.blue, expected.blue)
                && approx_eq(actual.alpha, expected.alpha),
            "actual={:?} expected={:?}",
            actual,
            expected,
        );
    }

    #[test]
    fn direct_color_var_emits_swatch() {
        let infos = collect_for(":root { --brand: #ff0000; } .x { color: var(--brand); }");
        assert_eq!(infos.len(), 1);
        assert_color(infos[0].color, red());
    }

    #[test]
    fn chained_var_resolves_to_color() {
        let infos = collect_for(":root { --a: var(--b); --b: #ff0000; } .x { color: var(--a); }");
        assert_eq!(infos.len(), 1);
        assert_color(infos[0].color, red());
    }

    #[test]
    fn non_color_var_emits_nothing() {
        let infos = collect_for(":root { --size: 16px; } .x { padding: var(--size); }");
        assert!(infos.is_empty());
    }

    #[test]
    fn undefined_var_emits_nothing() {
        let infos = collect_for(".x { color: var(--missing); }");
        assert!(infos.is_empty());
    }

    #[test]
    fn fallback_resolves_when_var_missing() {
        let infos = collect_for(".x { color: var(--missing, red); }");
        assert_eq!(infos.len(), 1);
        assert_color(infos[0].color, red());
    }

    #[test]
    fn definition_site_skipped_even_for_color() {
        let infos = collect_for(":root { --primary: var(--brand); --brand: #ff0000; }");
        assert!(infos.is_empty());
    }

    #[test]
    fn multiple_var_calls_in_one_property() {
        let infos = collect_for(
            ":root { --a: #ff0000; --b: #00ff00; } \
             .x { background: linear-gradient(var(--a), var(--b)); }",
        );
        assert_eq!(infos.len(), 2);
    }
}
