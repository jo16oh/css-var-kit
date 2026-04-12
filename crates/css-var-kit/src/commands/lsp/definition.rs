use std::error::Error;
use std::path::Path;

use lsp_server::{Message, Request, Response};
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range};

use super::Server;
use super::uri::path_to_uri;
use crate::commands::lint;
use crate::position::{byte_col_to_utf16_in_source, utf16_to_byte_offset};
use crate::searcher::SearcherBuilder;
use crate::searcher::conditions::variable_definitions::VariableDefinitions;

impl Server<'_> {
    pub fn handle_definition_request(&self, req: Request) -> Result<(), Box<dyn Error>> {
        let params: GotoDefinitionParams = serde_json::from_value(req.params)?;
        self.log(&format!(
            "textDocument/definition: {}",
            params
                .text_document_position_params
                .text_document
                .uri
                .as_str()
        ));
        let result = self.handle_definition(&params);
        let response = Response::new_ok(req.id, result);
        self.connection.sender.send(Message::Response(response))?;
        Ok(())
    }

    fn handle_definition(&self, params: &GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let source = self.open_documents.get(uri)?;
        let var_name = extract_variable_name_at_cursor(source, &pos)?;

        let sources: Vec<(&Path, &str)> = self
            .source_cache
            .iter()
            .map(|(path, content)| (path.as_path(), content.as_str()))
            .collect();

        let parse_results: Vec<_> = sources
            .iter()
            .flat_map(|(path, content)| lint::parse_file(content, path))
            .collect();

        let search_result = SearcherBuilder::new(&parse_results)
            .add_condition(VariableDefinitions::new(
                self.config.definition_files.clone(),
                self.config.include.clone(),
            ))
            .build()
            .search();

        let var_defs = search_result.get_prop_map_for::<VariableDefinitions>();
        let prop_id = lightningcss::properties::PropertyId::from(var_name.as_str());

        let props = var_defs.get(&prop_id)?;

        let locations: Vec<Location> = props
            .iter()
            .map(|prop| {
                let abs_path = self.config.root_dir.join(prop.file_path);
                let start = Position {
                    line: prop.name.line,
                    character: byte_col_to_utf16_in_source(
                        prop.source,
                        prop.name.line,
                        prop.name.column,
                    ),
                };
                let end = Position {
                    line: prop.name.line,
                    character: byte_col_to_utf16_in_source(
                        prop.source,
                        prop.name.line,
                        prop.name.column + prop.name.raw.len() as u32,
                    ),
                };
                Location {
                    uri: path_to_uri(&abs_path),
                    range: Range { start, end },
                }
            })
            .collect();

        if locations.is_empty() {
            None
        } else {
            Some(GotoDefinitionResponse::Array(locations))
        }
    }
}

pub struct VariableAtCursor {
    pub name: String,
    pub byte_start: usize,
    pub byte_end: usize,
}

pub fn extract_variable_at_cursor(source: &str, pos: &Position) -> Option<VariableAtCursor> {
    let line_str = source.lines().nth(pos.line as usize)?;
    let byte_col = utf16_to_byte_offset(line_str, pos.character);
    let bytes = line_str.as_bytes();

    let start = find_dashed_ident_start(bytes, byte_col)?;
    let end = find_dashed_ident_end(bytes, start);

    let name = &line_str[start..end];
    if name.len() >= 3 && name.starts_with("--") {
        Some(VariableAtCursor {
            name: name.to_owned(),
            byte_start: start,
            byte_end: end,
        })
    } else {
        None
    }
}

pub fn extract_variable_name_at_cursor(source: &str, pos: &Position) -> Option<String> {
    extract_variable_at_cursor(source, pos).map(|v| v.name)
}

fn find_dashed_ident_start(bytes: &[u8], byte_col: usize) -> Option<usize> {
    let mut i = byte_col.min(bytes.len());
    while i > 0 && is_ident_char(bytes[i - 1]) {
        i -= 1;
    }
    if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
        Some(i)
    } else {
        None
    }
}

fn find_dashed_ident_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && is_ident_char(bytes[i]) {
        i += 1;
    }
    i
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}
