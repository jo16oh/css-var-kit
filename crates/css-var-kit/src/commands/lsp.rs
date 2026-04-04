mod completion;
mod diagnostics;
mod file_watcher;
mod logger;
mod uri;

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crossbeam_channel::Receiver;
use lsp_server::{Connection, Message, Notification};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    Notification as _, PublishDiagnostics,
};
use lsp_types::{
    CompletionOptions, InitializeParams, PublishDiagnosticsParams, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};

use crate::commands::lint;
use crate::config::Config;
use logger::Logger;
use uri::uri_to_path;

pub fn run(cwd: &Path, log: bool) -> Result<(), Box<dyn Error>> {
    let (connection, _io_threads) = Connection::stdio();

    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec!["-".to_owned(), "(".to_owned()]),
            ..Default::default()
        }),
        ..Default::default()
    };

    let capabilities_json = serde_json::to_value(capabilities)?;
    let init_params: InitializeParams =
        serde_json::from_value(connection.initialize(capabilities_json)?)?;

    let root_dir = init_params
        .workspace_folders
        .as_ref()
        .and_then(|folders| folders.first())
        .and_then(|folder| uri_to_path(&folder.uri))
        .unwrap_or_else(|| cwd.to_path_buf());

    let config = Config::load(&root_dir, None)?;

    let logger = if log {
        let l = Logger::new(config.lsp_log_file.as_deref());
        l.log(&format!(
            "initialized: root_dir={}",
            config.root_dir.display()
        ));
        Some(l)
    } else {
        None
    };

    let watcher_rx = if file_watcher::client_supports_watch(&init_params) {
        file_watcher::register_client_watcher(&connection)?;
        if let Some(l) = &logger {
            l.log("watcher: using client-side (workspace/didChangeWatchedFiles)");
        }
        None
    } else {
        if let Some(l) = &logger {
            l.log("watcher: using server-side (notify crate)");
        }
        Some(file_watcher::start_server_watcher(&config.root_dir)?)
    };

    let source_cache = load_all_sources(&config);

    let mut server = Server {
        connection: &connection,
        config: &config,
        open_documents: HashMap::new(),
        source_cache,
        watcher_rx,
        logger: logger.as_ref(),
    };

    let result = server.main_loop();

    if let Some(l) = &logger {
        match &result {
            Ok(()) => l.log("shutdown"),
            Err(e) => l.log(&format!("error: {e}")),
        }
    }

    result
}

struct Server<'a> {
    connection: &'a Connection,
    config: &'a Config,
    open_documents: HashMap<Uri, String>,
    source_cache: HashMap<PathBuf, String>,
    watcher_rx: Option<Receiver<Vec<PathBuf>>>,
    logger: Option<&'a Logger>,
}

impl Server<'_> {
    fn main_loop(&mut self) -> Result<(), Box<dyn Error>> {
        let dummy_rx = crossbeam_channel::never();
        let watcher_rx = self.watcher_rx.take();
        let watcher_rx = watcher_rx.as_ref().unwrap_or(&dummy_rx);

        loop {
            crossbeam_channel::select! {
                recv(self.connection.receiver) -> msg => {
                    match msg? {
                        Message::Request(req) => {
                            if self.connection.handle_shutdown(&req)? {
                                return Ok(());
                            }
                            self.handle_request(req)?;
                        }
                        Message::Notification(notif) => self.handle_notification(notif)?,
                        Message::Response(_) => {}
                    }
                }
                recv(watcher_rx) -> paths => {
                    if let Ok(paths) = paths {
                        self.log(&format!(
                            "server watcher: file change detected ({} files)",
                            paths.len()
                        ));
                        self.update_source_cache_from_disk(&paths);
                        self.publish_diagnostics()?;
                    }
                }
            }
        }
    }

    fn handle_notification(&mut self, notif: Notification) -> Result<(), Box<dyn Error>> {
        match notif.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let params: lsp_types::DidOpenTextDocumentParams =
                    serde_json::from_value(notif.params)?;
                self.log(&format!(
                    "textDocument/didOpen: {}",
                    params.text_document.uri.as_str()
                ));
                if let Some(rel_path) = self.uri_to_rel_path(&params.text_document.uri) {
                    self.source_cache
                        .insert(rel_path, params.text_document.text.clone());
                }
                self.open_documents
                    .insert(params.text_document.uri, params.text_document.text);
                self.publish_diagnostics()?;
            }
            DidChangeTextDocument::METHOD => {
                let params: lsp_types::DidChangeTextDocumentParams =
                    serde_json::from_value(notif.params)?;
                self.log(&format!(
                    "textDocument/didChange: {} (version {})",
                    params.text_document.uri.as_str(),
                    params.text_document.version
                ));
                if let Some(change) = params.content_changes.into_iter().last() {
                    if let Some(rel_path) = self.uri_to_rel_path(&params.text_document.uri) {
                        self.source_cache.insert(rel_path, change.text.clone());
                    }
                    self.open_documents
                        .insert(params.text_document.uri, change.text);
                }
                self.publish_diagnostics()?;
            }
            DidChangeWatchedFiles::METHOD => {
                let params: lsp_types::DidChangeWatchedFilesParams =
                    serde_json::from_value(notif.params)?;
                let changed_paths: Vec<PathBuf> = params
                    .changes
                    .iter()
                    .filter_map(|change| uri_to_path(&change.uri))
                    .collect();
                self.log(&format!(
                    "workspace/didChangeWatchedFiles: {} files",
                    changed_paths.len()
                ));
                self.update_source_cache_from_disk(&changed_paths);
                self.publish_diagnostics()?;
            }
            DidCloseTextDocument::METHOD => {
                let params: lsp_types::DidCloseTextDocumentParams =
                    serde_json::from_value(notif.params)?;
                self.log(&format!(
                    "textDocument/didClose: {}",
                    params.text_document.uri.as_str()
                ));
                self.open_documents.remove(&params.text_document.uri);
                if let Some(rel_path) = self.uri_to_rel_path(&params.text_document.uri) {
                    match fs::read_to_string(self.config.root_dir.join(&rel_path)) {
                        Ok(content) => {
                            self.source_cache.insert(rel_path, content);
                        }
                        Err(_) => {
                            self.source_cache.remove(&rel_path);
                        }
                    }
                }
                self.send_notification::<PublishDiagnostics>(PublishDiagnosticsParams {
                    uri: params.text_document.uri,
                    diagnostics: vec![],
                    version: None,
                })?;
            }
            _ => {}
        }
        Ok(())
    }

    fn update_source_cache_from_disk(&mut self, abs_paths: &[PathBuf]) {
        for abs_path in abs_paths {
            let is_open = self
                .open_documents
                .keys()
                .any(|uri| uri_to_path(uri).as_deref() == Some(abs_path.as_path()));
            if is_open {
                continue;
            }

            let rel_path = abs_path
                .strip_prefix(&self.config.root_dir)
                .unwrap_or(abs_path)
                .to_path_buf();

            match fs::read_to_string(abs_path) {
                Ok(content) => {
                    self.source_cache.insert(rel_path, content);
                }
                Err(_) => {
                    self.source_cache.remove(&rel_path);
                }
            }
        }
    }

    fn uri_to_rel_path(&self, uri: &Uri) -> Option<PathBuf> {
        uri_to_path(uri).map(|abs_path| {
            abs_path
                .strip_prefix(&self.config.root_dir)
                .unwrap_or(&abs_path)
                .to_path_buf()
        })
    }

    fn log(&self, msg: &str) {
        if let Some(logger) = self.logger {
            logger.log(msg);
        }
    }

    fn send_notification<N: lsp_types::notification::Notification>(
        &self,
        params: N::Params,
    ) -> Result<(), Box<dyn Error>> {
        self.connection
            .sender
            .send(Message::Notification(Notification::new(
                N::METHOD.to_owned(),
                params,
            )))?;
        Ok(())
    }
}

fn load_all_sources(config: &Config) -> HashMap<PathBuf, String> {
    lint::collect_css_files(config.root_dir.as_path())
        .into_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok()?;
            let rel_path = path
                .strip_prefix(&config.root_dir)
                .unwrap_or(&path)
                .to_path_buf();
            Some((rel_path, content))
        })
        .collect()
}
