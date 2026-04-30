mod completion;
mod definition;
mod diagnostics;
mod file_watcher;
mod hover;
mod logger;
mod rename;
mod uri;
mod var_markdown;

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crossbeam_channel::Receiver;
use lsp_server::{Connection, Message, Notification};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    Notification as _, PublishDiagnostics,
};
use lsp_types::{
    CompletionOptions, DiagnosticOptions, DiagnosticServerCapabilities, HoverProviderCapability,
    InitializeParams, OneOf, PublishDiagnosticsParams, RenameOptions, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};

use crate::commands::lint;
use crate::config::{Config, RawConfig};
use crate::owned_types::OwnedStr;
use crate::parser::ParseResult;
use crate::searcher::Searcher;
use crate::searcher::conditions::non_custom_properties::NonCustomProperties;
use crate::searcher::conditions::variable_definitions::VariableDefinitions;
use crate::searcher::conditions::variable_usages::VariableUsages;
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
        definition_provider: Some(OneOf::Left(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        diagnostic_provider: Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
            inter_file_dependencies: true,
            workspace_diagnostics: true,
            ..Default::default()
        })),
        ..Default::default()
    };

    let capabilities_json = serde_json::to_value(capabilities)?;
    let mut init_params: InitializeParams =
        serde_json::from_value(connection.initialize(capabilities_json)?)?;

    let init_options: Option<RawConfig> = init_params
        .initialization_options
        .take()
        .and_then(|v| serde_json::from_value(v).ok());

    let root_dir = init_params
        .workspace_folders
        .as_ref()
        .and_then(|folders| folders.first())
        .and_then(|folder| uri_to_path(&folder.uri))
        .unwrap_or_else(|| cwd.to_path_buf());

    let config = Config::load_for_lsp(&root_dir, init_options.clone())?;

    let logger = (log || config.lsp_log_file.is_some()).then(|| {
        let l = Logger::new(config.lsp_log_file.as_deref());
        l.log(&format!(
            "initialized: root_dir={}",
            config.root_dir.display()
        ));
        l
    });

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
    let parse_cache = build_parse_cache(&source_cache);
    let searcher = build_searcher(&parse_cache, &config);

    let client_name = init_params
        .client_info
        .as_ref()
        .map(|info| info.name.clone());

    let mut server = Server {
        connection: &connection,
        config,
        lsp_root_dir: root_dir,
        init_options,
        client_name,
        opened_documents: HashMap::new(),
        source_cache,
        parse_cache,
        searcher,
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
    config: Config,
    lsp_root_dir: PathBuf,
    init_options: Option<RawConfig>,
    client_name: Option<String>,
    opened_documents: HashMap<Uri, String>,
    source_cache: HashMap<Rc<Path>, OwnedStr>,
    parse_cache: HashMap<Rc<Path>, Vec<ParseResult>>,
    searcher: Searcher,
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
                        let has_config_change = paths.iter().any(|p| is_config_file(p));
                        let source_paths: Vec<PathBuf> =
                            paths.into_iter().filter(|p| !is_config_file(p)).collect();
                        if has_config_change {
                            self.reload_config()?;
                        } else if !source_paths.is_empty() {
                            self.update_source_cache_from_disk(&source_paths);
                            self.publish_all_diagnostics()?;
                        }
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
                let rel_path = self.uri_to_rel_path(&params.text_document.uri);
                if let Some(ref rel_path) = rel_path {
                    let key = Rc::<Path>::from(rel_path.clone());
                    let source = OwnedStr::from(&params.text_document.text);
                    self.update_caches(&key, &source);
                    self.source_cache.insert(key, source);
                }
                self.opened_documents
                    .insert(params.text_document.uri, params.text_document.text);
                self.publish_all_diagnostics()?;
            }
            DidChangeTextDocument::METHOD => {
                let params: lsp_types::DidChangeTextDocumentParams =
                    serde_json::from_value(notif.params)?;
                self.log(&format!(
                    "textDocument/didChange: {} (version {})",
                    params.text_document.uri.as_str(),
                    params.text_document.version
                ));
                let mut changed_rel_path = None;
                if let Some(change) = params.content_changes.into_iter().last() {
                    if let Some(rel_path) = self.uri_to_rel_path(&params.text_document.uri) {
                        let key = Rc::<Path>::from(rel_path.clone());
                        let source = OwnedStr::from(&change.text);
                        self.update_caches(&key, &source);
                        self.source_cache.insert(key, source);
                        changed_rel_path = Some(rel_path);
                    }
                    self.opened_documents
                        .insert(params.text_document.uri, change.text);
                }
                if changed_rel_path
                    .as_deref()
                    .is_some_and(|p| self.is_definition_file(p))
                {
                    self.publish_all_diagnostics()?;
                } else {
                    let targets: Vec<Rc<Path>> = changed_rel_path
                        .map(|p| vec![Rc::<Path>::from(p)])
                        .unwrap_or_default();
                    self.publish_diagnostics_for_files(&targets)?;
                }
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
                let has_config_change = changed_paths.iter().any(|p| is_config_file(p));
                let source_paths: Vec<PathBuf> = changed_paths
                    .into_iter()
                    .filter(|p| !is_config_file(p))
                    .collect();
                if has_config_change {
                    self.reload_config()?;
                } else if !source_paths.is_empty() {
                    self.update_source_cache_from_disk(&source_paths);
                    self.publish_all_diagnostics()?;
                }
            }
            DidCloseTextDocument::METHOD => {
                let params: lsp_types::DidCloseTextDocumentParams =
                    serde_json::from_value(notif.params)?;
                self.log(&format!(
                    "textDocument/didClose: {}",
                    params.text_document.uri.as_str()
                ));
                self.opened_documents.remove(&params.text_document.uri);
                if let Some(rel_path) = self.uri_to_rel_path(&params.text_document.uri) {
                    match fs::read_to_string(self.config.root_dir.join(&rel_path)) {
                        Ok(content) => {
                            let key = Rc::<Path>::from(rel_path);
                            let source = OwnedStr::from(content);
                            self.update_caches(&key, &source);
                            self.source_cache.insert(key, source);
                        }
                        Err(_) => {
                            self.remove_from_caches(rel_path.as_path());
                            self.source_cache.remove(rel_path.as_path());
                            self.send_notification::<PublishDiagnostics>(
                                PublishDiagnosticsParams {
                                    uri: params.text_document.uri,
                                    diagnostics: vec![],
                                    version: None,
                                },
                            )?;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn update_source_cache_from_disk(&mut self, abs_paths: &[PathBuf]) {
        for abs_path in abs_paths {
            let is_open = self
                .opened_documents
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
                    let key = Rc::<Path>::from(rel_path);
                    let source = OwnedStr::from(content);
                    self.update_caches(&key, &source);
                    self.source_cache.insert(key, source);
                }
                Err(_) => {
                    self.remove_from_caches(rel_path.as_path());
                    self.source_cache.remove(rel_path.as_path());
                }
            }
        }
    }

    fn update_caches(&mut self, path: &Rc<Path>, source: &OwnedStr) {
        let parse_results = lint::parse_file(source, path);
        self.searcher.update_file(path, &parse_results);
        self.parse_cache.insert(path.clone(), parse_results);
    }

    fn remove_from_caches(&mut self, path: &Path) {
        self.searcher.remove_file(path);
        self.parse_cache.remove(path);
    }

    fn is_definition_file(&self, rel_path: &Path) -> bool {
        self.config.definition_files.matches(&rel_path) || self.config.include.matches(&rel_path)
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

    fn reload_config(&mut self) -> Result<(), Box<dyn Error>> {
        match Config::load_for_lsp(&self.lsp_root_dir, self.init_options.clone()) {
            Ok(new_config) => {
                self.config = new_config;
                self.source_cache = load_all_sources(&self.config);
                self.parse_cache = build_parse_cache(&self.source_cache);
                self.searcher = build_searcher(&self.parse_cache, &self.config);
                self.log("config reloaded");
                self.publish_all_diagnostics()?;
            }
            Err(e) => {
                self.log(&format!("config reload failed: {e}"));
            }
        }
        Ok(())
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

fn is_config_file(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("cvk.json" | "cvk.jsonc")
    )
}

fn build_searcher(parse_cache: &HashMap<Rc<Path>, Vec<ParseResult>>, config: &Config) -> Searcher {
    let mut searcher = Searcher::new()
        .add_condition(VariableDefinitions::new(
            config.definition_files.clone(),
            config.include.clone(),
        ))
        .add_condition(VariableUsages)
        .add_condition(NonCustomProperties);

    for (path, parse_results) in parse_cache {
        searcher.update_file(path, parse_results);
    }

    searcher
}

fn build_parse_cache(
    source_cache: &HashMap<Rc<Path>, OwnedStr>,
) -> HashMap<Rc<Path>, Vec<ParseResult>> {
    source_cache
        .iter()
        .map(|(path, content)| (path.clone(), lint::parse_file(content, path)))
        .collect()
}

fn load_all_sources(config: &Config) -> HashMap<Rc<Path>, OwnedStr> {
    let lint_sources = lint::collect_source_files(config.root_dir.as_path(), &config.include)
        .into_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok().map(OwnedStr::from)?;
            let rel_path = path
                .strip_prefix(&config.root_dir)
                .unwrap_or(&path)
                .to_path_buf();
            if config.include.is_negated(&rel_path) {
                return None;
            }
            Some((Rc::<Path>::from(rel_path), content))
        });

    let include_sources = lint::collect_include_files(config.root_dir.as_path(), &config.include)
        .into_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok().map(OwnedStr::from)?;
            let rel_path = path
                .strip_prefix(&config.root_dir)
                .unwrap_or(&path)
                .to_path_buf();
            Some((Rc::from(rel_path), content))
        });

    lint_sources.chain(include_sources).collect()
}
