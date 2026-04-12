use std::error::Error;
use std::path::Path;

use lsp_server::{Message, Request, Response};
use lsp_types::request::{Completion, GotoDefinition, PrepareRenameRequest, Rename};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, CompletionTextEdit,
    Position, Range, TextEdit,
};

use super::Server;
use crate::commands::lint;
use crate::position::{byte_offset_to_utf16, utf16_to_byte_offset};
use crate::searcher::SearcherBuilder;
use crate::searcher::conditions::variable_definitions::VariableDefinitions;
use crate::type_checker::{TypeCheckError, check_property_type};

impl Server<'_> {
    pub fn handle_request(&self, req: Request) -> Result<(), Box<dyn Error>> {
        match req.method.as_str() {
            <Completion as lsp_types::request::Request>::METHOD => {
                let params: CompletionParams = serde_json::from_value(req.params)?;
                self.log(&format!(
                    "textDocument/completion: {}",
                    params.text_document_position.text_document.uri.as_str()
                ));
                let result = self.handle_completion(&params);
                let response = Response::new_ok(req.id, result);
                self.connection.sender.send(Message::Response(response))?;
            }
            <GotoDefinition as lsp_types::request::Request>::METHOD => {
                self.handle_definition_request(req)?;
            }
            <Rename as lsp_types::request::Request>::METHOD => {
                self.handle_rename_request(req)?;
            }
            <PrepareRenameRequest as lsp_types::request::Request>::METHOD => {
                self.handle_prepare_rename_request(req)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_completion(&self, params: &CompletionParams) -> Option<CompletionResponse> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let source = self.open_documents.get(uri)?;
        let ctx = extract_property_context(source, &pos)?;

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
            .build()
            .search();

        let var_defs = search_result.get_prop_map_for::<VariableDefinitions>();
        let vars = var_defs.vars_map();

        let replace_range = Range {
            start: Position {
                line: pos.line,
                character: ctx.replace_start,
            },
            end: pos,
        };

        let items: Vec<CompletionItem> = var_defs
            .iter()
            .filter(|(_prop_id, props)| {
                let name = props[0].name.raw;
                let test_value = build_test_value(&ctx, name);
                !matches!(
                    check_property_type(&ctx.property_name, &test_value, &vars),
                    Err(TypeCheckError::TypeMismatch(_, _))
                )
            })
            .map(|(_prop_id, props)| {
                let name = props[0].name.raw;
                let detail = props.last().map(|p| p.value.raw.to_owned());
                let new_text = if ctx.inside_var {
                    name.to_owned()
                } else {
                    format!("var({name})")
                };
                CompletionItem {
                    label: name.to_owned(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail,
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: replace_range,
                        new_text,
                    })),
                    filter_text: Some(name.to_owned()),
                    ..Default::default()
                }
            })
            .collect();

        self.log(&format!("completion: {} items", items.len()));
        Some(CompletionResponse::Array(items))
    }
}

struct PropertyContext {
    property_name: String,
    value_prefix: String,
    value_suffix: String,
    inside_var: bool,
    replace_start: u32,
}

fn extract_property_context(source: &str, pos: &Position) -> Option<PropertyContext> {
    let line_str = source.lines().nth(pos.line as usize)?;
    let byte_col = utf16_to_byte_offset(line_str, pos.character);
    let before_cursor = &line_str[..byte_col.min(line_str.len())];

    let colon_pos = before_cursor.rfind(':')?;

    let is_after_colon = [before_cursor.rfind(';'), before_cursor.rfind('{')]
        .into_iter()
        .flatten()
        .all(|p| p < colon_pos);
    if !is_after_colon {
        return None;
    }

    let before_colon = before_cursor[..colon_pos].trim();
    let property_name = before_colon.rsplit([';', '{', '}']).next()?.trim();

    if property_name.is_empty() {
        return None;
    }

    let after_colon = &before_cursor[colon_pos + 1..];
    let leading_ws = after_colon.len() - after_colon.trim_start().len();
    let value_start_byte = colon_pos + 1 + leading_ws;
    let value_prefix = after_colon.trim_start().to_owned();
    let inside_var = is_inside_var(&value_prefix);

    let after_cursor = &line_str[byte_col.min(line_str.len())..];
    let rest_of_token = after_cursor
        .find(|c: char| c.is_whitespace() || matches!(c, ';' | '}'))
        .unwrap_or(after_cursor.len());
    let after_token = &after_cursor[rest_of_token..];
    let value_suffix = after_token
        .find([';', '}'])
        .map(|end| after_token[..end].to_owned())
        .unwrap_or_else(|| after_token.to_owned());

    let token_start_in_prefix = if inside_var {
        value_prefix.rfind("var(").map(|p| p + 4).unwrap_or(0)
    } else {
        value_prefix
            .rfind(|c: char| c.is_whitespace() || c == '(' || c == ',')
            .map(|i| i + 1)
            .unwrap_or(0)
    };

    let token_start_byte = value_start_byte + token_start_in_prefix;
    let replace_start = byte_offset_to_utf16(line_str, token_start_byte);

    Some(PropertyContext {
        property_name: property_name.to_owned(),
        value_prefix,
        value_suffix,
        inside_var,
        replace_start,
    })
}

fn is_inside_var(value_prefix: &str) -> bool {
    value_prefix
        .rfind("var(")
        .is_some_and(|pos| !value_prefix[pos + 4..].contains(')'))
}

fn build_test_value(ctx: &PropertyContext, var_name: &str) -> String {
    let suffix = &ctx.value_suffix;
    if ctx.inside_var {
        let var_start = ctx.value_prefix.rfind("var(").unwrap_or(0);
        let before_var = &ctx.value_prefix[..var_start];
        format!("{before_var}var({var_name}){suffix}")
    } else {
        let token_start = ctx
            .value_prefix
            .rfind(|c: char| c.is_whitespace() || c == '(' || c == ',')
            .map(|i| i + 1)
            .unwrap_or(0);
        let context = &ctx.value_prefix[..token_start];
        format!("{context}var({var_name}){suffix}")
    }
}
