use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use lsp_server::{Message, Request, Response};
use lsp_types::{
    DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
    FullDocumentDiagnosticReport, RelatedFullDocumentDiagnosticReport, WorkspaceDiagnosticParams,
    WorkspaceDiagnosticReport, WorkspaceDiagnosticReportResult, WorkspaceDocumentDiagnosticReport,
    WorkspaceFullDocumentDiagnosticReport,
};

use super::Server;
use super::push_diagnostics::to_lsp_diagnostic;
use super::uri::path_to_uri;
use crate::commands::lint;

impl Server<'_> {
    pub fn handle_document_diagnostic_request(&self, req: Request) -> Result<(), Box<dyn Error>> {
        let params: DocumentDiagnosticParams = serde_json::from_value(req.params)?;
        self.log(&format!(
            "textDocument/diagnostic: {}",
            params.text_document.uri.as_str()
        ));
        let result = self.document_diagnostic_report(&params);
        let response = Response::new_ok(req.id, result);
        self.connection.sender.send(Message::Response(response))?;
        Ok(())
    }

    pub fn handle_workspace_diagnostic_request(&self, req: Request) -> Result<(), Box<dyn Error>> {
        let _params: WorkspaceDiagnosticParams = serde_json::from_value(req.params)?;
        self.log("workspace/diagnostic");
        let result = self.workspace_diagnostic_report();
        let response = Response::new_ok(req.id, result);
        self.connection.sender.send(Message::Response(response))?;
        Ok(())
    }

    fn document_diagnostic_report(
        &self,
        params: &DocumentDiagnosticParams,
    ) -> DocumentDiagnosticReportResult {
        let items = self
            .uri_to_rel_path(&params.text_document.uri)
            .map(|p| diagnostics_for_single_file(self, &Rc::<Path>::from(p)))
            .unwrap_or_default();

        DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
            RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items,
                },
            },
        ))
    }

    fn workspace_diagnostic_report(&self) -> WorkspaceDiagnosticReportResult {
        let search_result = self.searcher.search();
        let diagnostics = lint::check_search_result(&search_result, &self.config);

        let mut by_file: HashMap<Rc<Path>, Vec<lsp_types::Diagnostic>> = HashMap::new();
        for d in &diagnostics {
            by_file
                .entry(d.file_path.clone())
                .or_default()
                .push(to_lsp_diagnostic(d));
        }

        let items: Vec<WorkspaceDocumentDiagnosticReport> = self
            .source_cache
            .keys()
            .filter(|path| !self.config.include.is_negated(path))
            .map(|path| {
                let lsp_diags = by_file.remove(path).unwrap_or_default();
                let uri = path_to_uri(&self.config.root_dir.join(path.as_ref()));
                WorkspaceDocumentDiagnosticReport::Full(WorkspaceFullDocumentDiagnosticReport {
                    uri,
                    version: None,
                    full_document_diagnostic_report: FullDocumentDiagnosticReport {
                        result_id: None,
                        items: lsp_diags,
                    },
                })
            })
            .collect();

        self.log(&format!("workspace/diagnostic: {} files", items.len()));

        WorkspaceDiagnosticReportResult::Report(WorkspaceDiagnosticReport { items })
    }
}

fn diagnostics_for_single_file(
    server: &Server<'_>,
    rel_path: &Rc<Path>,
) -> Vec<lsp_types::Diagnostic> {
    let target_set: HashSet<Rc<Path>> = [rel_path.clone()].into();
    let search_result = server.searcher.search_for_files(&target_set);
    lint::check_search_result(&search_result, &server.config)
        .iter()
        .filter(|d| d.file_path == *rel_path)
        .map(to_lsp_diagnostic)
        .collect()
}
