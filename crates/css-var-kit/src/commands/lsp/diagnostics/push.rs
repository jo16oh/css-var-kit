use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use lsp_types::PublishDiagnosticsParams;
use lsp_types::notification::PublishDiagnostics;

use super::Server;
use super::to_lsp_diagnostic;
use crate::commands::lint;
use crate::commands::lsp::uri::path_to_uri;

impl Server<'_> {
    pub fn publish_all_diagnostics(&self) -> Result<(), Box<dyn Error>> {
        let source_paths: Vec<Rc<Path>> = self
            .source_cache
            .keys()
            .filter(|path| !self.config.include.is_negated(path))
            .cloned()
            .collect();

        let search_result = self.searcher.search();
        let diagnostics = lint::check_search_result(&search_result, &self.config);

        self.log(&format!(
            "publishDiagnostics: {} files (all), {} diagnostics total",
            source_paths.len(),
            diagnostics.len()
        ));

        let mut by_file: HashMap<Rc<Path>, Vec<lsp_types::Diagnostic>> = HashMap::new();
        for d in &diagnostics {
            by_file
                .entry(d.file_path.clone())
                .or_default()
                .push(to_lsp_diagnostic(d));
        }

        for path in &source_paths {
            let lsp_diagnostics = by_file.remove(path).unwrap_or_default();
            let abs_path = self.config.root_dir.join(path.as_ref());
            let uri = path_to_uri(&abs_path);
            self.send_notification::<PublishDiagnostics>(PublishDiagnosticsParams {
                uri,
                diagnostics: lsp_diagnostics,
                version: None,
            })?;
        }

        Ok(())
    }

    pub fn publish_diagnostics_for_files(
        &self,
        target_paths: &[Rc<Path>],
    ) -> Result<(), Box<dyn Error>> {
        let target_set: HashSet<Rc<Path>> = target_paths
            .iter()
            .filter(|path| !self.config.include.is_negated(path))
            .cloned()
            .collect();

        let search_result = self.searcher.search_for_files(&target_set);
        let diagnostics = lint::check_search_result(&search_result, &self.config);

        self.log(&format!(
            "publishDiagnostics: {} files (targeted), {} diagnostics",
            target_set.len(),
            diagnostics.len()
        ));

        let mut by_file: HashMap<Rc<Path>, Vec<lsp_types::Diagnostic>> = HashMap::new();
        for d in &diagnostics {
            by_file
                .entry(d.file_path.clone())
                .or_default()
                .push(to_lsp_diagnostic(d));
        }

        for path in target_set.iter() {
            let lsp_diagnostics = by_file.remove(path.as_ref()).unwrap_or_default();
            let abs_path = self.config.root_dir.join(path.as_ref());
            let uri = path_to_uri(&abs_path);
            self.send_notification::<PublishDiagnostics>(PublishDiagnosticsParams {
                uri,
                diagnostics: lsp_diagnostics,
                version: None,
            })?;
        }

        Ok(())
    }
}
