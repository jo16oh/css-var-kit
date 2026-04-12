use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use lsp_server::{Message, Request, Response};
use lsp_types::{
    PrepareRenameResponse, RenameParams, TextDocumentPositionParams, TextEdit, Uri, WorkspaceEdit,
};

use super::Server;
use super::definition::{extract_variable_at_cursor, extract_variable_name_at_cursor};
use super::uri::path_to_uri;
use crate::commands::lint;
use crate::position::offset_to_position;
use crate::position::{byte_col_to_utf16_in_source, byte_offset_to_utf16};
use crate::searcher::SearcherBuilder;
use crate::searcher::conditions::variable_definitions::VariableDefinitions;
use crate::searcher::conditions::variable_usages::VariableUsages;

impl Server<'_> {
    pub fn handle_rename_request(&self, req: Request) -> Result<(), Box<dyn Error>> {
        let params: RenameParams = serde_json::from_value(req.params)?;
        self.log(&format!(
            "textDocument/rename: {} -> {}",
            params.text_document_position.text_document.uri.as_str(),
            params.new_name
        ));
        let result = self.handle_rename(&params);
        let response = Response::new_ok(req.id, result);
        self.connection.sender.send(Message::Response(response))?;
        Ok(())
    }

    pub fn handle_prepare_rename_request(&self, req: Request) -> Result<(), Box<dyn Error>> {
        let params: TextDocumentPositionParams = serde_json::from_value(req.params)?;
        self.log(&format!(
            "textDocument/prepareRename: {}",
            params.text_document.uri.as_str()
        ));
        let result = self.handle_prepare_rename(&params);
        let response = Response::new_ok(req.id, result);
        self.connection.sender.send(Message::Response(response))?;
        Ok(())
    }

    fn handle_prepare_rename(
        &self,
        params: &TextDocumentPositionParams,
    ) -> Option<PrepareRenameResponse> {
        let source = self.open_documents.get(&params.text_document.uri)?;
        let var = extract_variable_at_cursor(source, &params.position)?;
        let line_str = source.lines().nth(params.position.line as usize)?;
        let start_char = byte_offset_to_utf16(line_str, var.byte_start);
        let end_char = byte_offset_to_utf16(line_str, var.byte_end);
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: params.position.line,
                    character: start_char,
                },
                end: lsp_types::Position {
                    line: params.position.line,
                    character: end_char,
                },
            },
            placeholder: var.name.strip_prefix("--").unwrap_or(&var.name).to_owned(),
        })
    }

    fn handle_rename(&self, params: &RenameParams) -> Option<WorkspaceEdit> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = &params.new_name;

        let source = self.open_documents.get(uri)?;
        let old_name = extract_variable_name_at_cursor(source, &pos)?;

        let new_name = if new_name.starts_with("--") {
            new_name.to_owned()
        } else {
            format!("--{new_name}")
        };

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
            .add_condition(VariableDefinitions::new(self.config.lookup_files.clone()))
            .add_condition(VariableUsages)
            .build()
            .search();

        #[allow(clippy::mutable_key_type)]
        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();

        let def_map = search_result.get_prop_map_for::<VariableDefinitions>();
        let prop_id = lightningcss::properties::PropertyId::from(old_name.as_str());

        if let Some(defs) = def_map.get(&prop_id) {
            for prop in &defs {
                let abs_path = self.config.root_dir.join(prop.file_path);
                let file_uri = path_to_uri(&abs_path);
                let edit = make_text_edit(
                    prop.source,
                    prop.name.line,
                    prop.name.column,
                    old_name.len() as u32,
                    &new_name,
                );
                changes.entry(file_uri).or_default().push(edit);
            }
        }

        let usages = search_result.get_result_for(VariableUsages);
        for prop in usages.iter() {
            collect_usage_edits(prop, &old_name, &new_name, self, &mut changes);
        }

        if changes.is_empty() {
            return None;
        }

        self.log(&format!(
            "rename: {} -> {}, {} files",
            old_name,
            new_name,
            changes.len()
        ));

        Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        })
    }
}

#[allow(clippy::mutable_key_type)]
fn collect_usage_edits(
    prop: &crate::searcher::Property<'_>,
    old_name: &str,
    new_name: &str,
    server: &Server<'_>,
    changes: &mut HashMap<Uri, Vec<TextEdit>>,
) {
    let abs_path = server.config.root_dir.join(prop.file_path);
    let file_uri = path_to_uri(&abs_path);

    let mut search_from = 0usize;
    collect_from_token_list(
        prop.token_list(),
        prop,
        old_name,
        new_name,
        &file_uri,
        &mut search_from,
        changes,
    );
}

#[allow(clippy::mutable_key_type)]
fn collect_from_token_list(
    token_list: &lightningcss::properties::custom::TokenList<'_>,
    prop: &crate::searcher::Property<'_>,
    old_name: &str,
    new_name: &str,
    file_uri: &Uri,
    search_from: &mut usize,
    changes: &mut HashMap<Uri, Vec<TextEdit>>,
) {
    use lightningcss::properties::custom::TokenOrValue;

    for token in &token_list.0 {
        match token {
            TokenOrValue::Var(var) => {
                let name = &*var.name.ident.0;
                if let Some((line, col, len, next_sf)) =
                    find_var_in_source(prop.source, prop.value.offset, *search_from, name)
                {
                    if name == old_name {
                        let edit = make_text_edit(prop.source, line, col, len, new_name);
                        changes.entry(file_uri.clone()).or_default().push(edit);
                    }
                    *search_from = next_sf;
                }
                if let Some(fallback) = &var.fallback {
                    collect_from_token_list(
                        fallback,
                        prop,
                        old_name,
                        new_name,
                        file_uri,
                        search_from,
                        changes,
                    );
                }
            }
            TokenOrValue::DashedIdent(ident) => {
                let name = &*ident.0;
                if let Some((line, col, len, next_sf)) =
                    find_var_in_source(prop.source, prop.value.offset, *search_from, name)
                {
                    if name == old_name {
                        let edit = make_text_edit(prop.source, line, col, len, new_name);
                        changes.entry(file_uri.clone()).or_default().push(edit);
                    }
                    *search_from = next_sf;
                }
            }
            TokenOrValue::Function(func) => {
                collect_from_token_list(
                    &func.arguments,
                    prop,
                    old_name,
                    new_name,
                    file_uri,
                    search_from,
                    changes,
                );
            }
            _ => {}
        }
    }
}

fn find_var_in_source(
    source: &str,
    value_offset: usize,
    search_from: usize,
    var_name: &str,
) -> Option<(u32, u32, u32, usize)> {
    let search_start = value_offset + search_from;
    source.get(search_start..).and_then(|haystack| {
        haystack.find(var_name).map(|pos| {
            let abs_offset = search_start + pos;
            let (line, column) = offset_to_position(source, abs_offset);
            (
                line,
                column,
                var_name.len() as u32,
                search_from + pos + var_name.len(),
            )
        })
    })
}

fn make_text_edit(
    source: &str,
    line: u32,
    byte_col: u32,
    byte_len: u32,
    new_text: &str,
) -> TextEdit {
    let start_char = byte_col_to_utf16_in_source(source, line, byte_col);
    let end_char = byte_col_to_utf16_in_source(source, line, byte_col + byte_len);
    TextEdit {
        range: lsp_types::Range {
            start: lsp_types::Position {
                line,
                character: start_char,
            },
            end: lsp_types::Position {
                line,
                character: end_char,
            },
        },
        new_text: new_text.to_owned(),
    }
}
