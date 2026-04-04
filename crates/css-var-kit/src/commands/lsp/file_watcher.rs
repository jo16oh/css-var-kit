use std::error::Error;
use std::path::Path;

use crossbeam_channel::Receiver;
use lsp_server::{Connection, Message, Request};
use lsp_types::notification::DidChangeWatchedFiles;
use lsp_types::notification::Notification as _;
use lsp_types::request::{RegisterCapability, Request as _};
use lsp_types::{
    DidChangeWatchedFilesRegistrationOptions, FileSystemWatcher, GlobPattern, InitializeParams,
    Registration,
};
use notify::{RecursiveMode, Watcher};

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

pub fn start_server_watcher(root_dir: &Path) -> Result<Receiver<()>, Box<dyn Error>> {
    let (tx, rx) = crossbeam_channel::bounded(1);

    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            let is_css = event
                .paths
                .iter()
                .any(|p| p.extension().is_some_and(|ext| ext == "css"));
            if is_css {
                let _ = tx.try_send(());
            }
        }
    })?;

    watcher.watch(root_dir, RecursiveMode::Recursive)?;

    std::thread::spawn(move || {
        let _watcher = watcher;
        std::thread::park();
    });

    Ok(rx)
}
