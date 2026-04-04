use std::error::Error;
use std::path::Path;

use lsp_server::{Message, Request, Response};
use lsp_types::request::Completion;
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, Position,
};

use super::Server;
use crate::parser;
use crate::searcher::SearcherBuilder;
use crate::searcher::conditions::variable_definitions::VariableDefinitions;

impl Server<'_> {
    pub fn handle_request(&self, req: Request) -> Result<(), Box<dyn Error>> {
        if let <Completion as lsp_types::request::Request>::METHOD = req.method.as_str() {
            let params: CompletionParams = serde_json::from_value(req.params)?;
            self.log(&format!(
                "textDocument/completion: {}",
                params.text_document_position.text_document.uri.as_str()
            ));
            let result = self.handle_completion(&params);
            let response = Response::new_ok(req.id, result);
            self.connection.sender.send(Message::Response(response))?;
        }
        Ok(())
    }

    fn handle_completion(&self, params: &CompletionParams) -> Option<CompletionResponse> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let source = self.open_documents.get(uri)?;
        if !is_in_property_value(source, &pos) {
            return None;
        }

        let sources: Vec<(&Path, &str)> = self
            .source_cache
            .iter()
            .map(|(path, content)| (path.as_path(), content.as_str()))
            .collect();

        let parse_results: Vec<_> = sources
            .iter()
            .map(|(path, content)| parser::css::parse(content, path))
            .collect();

        let search_result = SearcherBuilder::new(&parse_results)
            .add_condition(VariableDefinitions)
            .build()
            .search();

        let var_defs = search_result.get_prop_map_for::<VariableDefinitions>();

        let items: Vec<CompletionItem> = var_defs
            .iter()
            .map(|(_prop_id, props)| {
                let name = props[0].name.raw;
                let detail = props.last().map(|p| p.value.raw.to_owned());
                CompletionItem {
                    label: name.to_owned(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail,
                    insert_text: Some(format!("var({name})")),
                    filter_text: Some(name.to_owned()),
                    ..Default::default()
                }
            })
            .collect();

        self.log(&format!("completion: {} items", items.len()));
        Some(CompletionResponse::Array(items))
    }
}

fn is_in_property_value(source: &str, pos: &Position) -> bool {
    let line_str = match source.lines().nth(pos.line as usize) {
        Some(l) => l,
        None => return false,
    };

    let byte_col = utf16_to_byte_offset(line_str, pos.character);
    let before_cursor = &line_str[..byte_col.min(line_str.len())];

    let last_colon = before_cursor.rfind(':');
    let last_semicolon = before_cursor.rfind(';');
    let last_open_brace = before_cursor.rfind('{');

    match last_colon {
        Some(colon_pos) => [last_semicolon, last_open_brace]
            .into_iter()
            .flatten()
            .all(|p| p < colon_pos),
        None => false,
    }
}

fn utf16_to_byte_offset(line: &str, utf16_col: u32) -> usize {
    let mut utf16_count = 0u32;
    for (byte_idx, ch) in line.char_indices() {
        if utf16_count >= utf16_col {
            return byte_idx;
        }
        utf16_count += ch.len_utf16() as u32;
    }
    line.len()
}
