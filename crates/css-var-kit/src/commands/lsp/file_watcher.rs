use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossbeam_channel::Receiver;
use lsp_server::{Connection, Message, Request};
use lsp_types::notification::DidChangeWatchedFiles;
use lsp_types::notification::Notification as _;
use lsp_types::request::{RegisterCapability, Request as _};
use lsp_types::{
    DidChangeWatchedFilesRegistrationOptions, FileSystemWatcher, GlobPattern, InitializeParams,
    Registration,
};
use notify::RecursiveMode;
use notify_debouncer_full::{DebouncedEvent, new_debouncer};

pub fn client_supports_watch(init_params: &InitializeParams) -> bool {
    init_params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|ws| ws.did_change_watched_files.as_ref())
        .and_then(|cap| cap.dynamic_registration)
        .unwrap_or(false)
}

pub fn register_client_watcher(connection: &Connection) -> Result<(), Box<dyn Error>> {
    let registration = Registration {
        id: "css-watcher".to_owned(),
        method: DidChangeWatchedFiles::METHOD.to_owned(),
        register_options: Some(serde_json::to_value(
            DidChangeWatchedFilesRegistrationOptions {
                watchers: vec![FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/*.css".to_owned()),
                    kind: None,
                }],
            },
        )?),
    };

    let params = lsp_types::RegistrationParams {
        registrations: vec![registration],
    };

    let req = Request::new(1.into(), RegisterCapability::METHOD.to_owned(), params);
    connection.sender.send(Message::Request(req))?;

    Ok(())
}

pub fn start_server_watcher(root_dir: &Path) -> Result<Receiver<Vec<PathBuf>>, Box<dyn Error>> {
    let (tx, rx) = crossbeam_channel::bounded(1);

    let mut debouncer = new_debouncer(
        Duration::from_millis(1000),
        None,
        move |events: Result<Vec<DebouncedEvent>, _>| {
            eprintln!("--- Debouncer fired! ---"); // デバッグ用
            if let Ok(events) = events {
                let css_paths: Vec<PathBuf> = events
                    .iter()
                    .flat_map(|e| &e.paths)
                    .filter(|p| p.extension().is_some_and(|ext| ext == "css"))
                    .cloned()
                    .collect();

                if !css_paths.is_empty() {
                    let _ = tx.try_send(css_paths);
                }
            }
        },
    )?;

    let root_dir = root_dir.to_owned();

    std::thread::spawn(move || {
        debouncer.watch(root_dir, RecursiveMode::Recursive).unwrap();
        std::thread::park();
    });

    Ok(rx)
}
