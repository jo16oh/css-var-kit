#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};

pub struct LspClient {
    stdin: std::process::ChildStdin,
    messages: mpsc::Receiver<Value>,
    process: std::process::Child,
    request_id: i64,
    root_uri: String,
}

pub struct PublishedDiagnostics {
    pub uri: String,
    pub diagnostics: Vec<DiagnosticInfo>,
}

pub struct DiagnosticInfo {
    pub message: String,
    pub line: u64,
    pub character: u64,
}

impl LspClient {
    pub fn spawn(workspace: &Path) -> Self {
        Self::spawn_with_args(workspace, &[])
    }

    pub fn spawn_with_args(workspace: &Path, extra_args: &[&str]) -> Self {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cvk"));
        cmd.arg("lsp")
            .args(extra_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .current_dir(workspace);
        let mut process = cmd.spawn().expect("failed to spawn cvk lsp");

        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(content_length) = read_header(&mut reader) {
                let mut body = vec![0u8; content_length];
                if reader.read_exact(&mut body).is_err() {
                    break;
                }

                if let Ok(msg) = serde_json::from_slice::<Value>(&body) {
                    if tx.send(msg).is_err() {
                        break;
                    }
                }
            }
        });

        let root_uri = format!("file://{}", workspace.canonicalize().unwrap().display());

        Self {
            stdin,
            messages: rx,
            process,
            request_id: 0,
            root_uri,
        }
    }

    pub fn initialize(&mut self) {
        self.initialize_with_options(None);
    }

    pub fn initialize_with_options(&mut self, init_options: Option<Value>) {
        let mut params = json!({
            "capabilities": {},
            "workspaceFolders": [{
                "uri": self.root_uri,
                "name": "test"
            }]
        });
        if let Some(opts) = init_options {
            params["initializationOptions"] = opts;
        }
        let response = self.send_request("initialize", params);
        assert!(
            response.get("result").is_some(),
            "initialize failed: {response}"
        );

        self.send_notification("initialized", json!({}));
    }

    pub fn open_document(&mut self, uri: &str, text: &str) {
        self.send_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "css",
                    "version": 1,
                    "text": text
                }
            }),
        );
    }

    pub fn change_document(&mut self, uri: &str, version: i32, text: &str) {
        self.send_notification(
            "textDocument/didChange",
            json!({
                "textDocument": {
                    "uri": uri,
                    "version": version
                },
                "contentChanges": [{
                    "text": text
                }]
            }),
        );
    }

    pub fn notify_watched_files_changed(&mut self, uris: &[&str]) {
        let changes: Vec<Value> = uris
            .iter()
            .map(|uri| {
                json!({
                    "uri": uri,
                    "type": 2 // FileChangeType.Changed
                })
            })
            .collect();
        self.send_notification(
            "workspace/didChangeWatchedFiles",
            json!({ "changes": changes }),
        );
    }

    pub fn file_uri(&self, relative_path: &str) -> String {
        format!("{}/{relative_path}", self.root_uri)
    }

    pub fn request_completion(&mut self, uri: &str, line: u32, character: u32) -> Value {
        self.send_request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        )
    }

    pub fn request_rename(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Value {
        self.send_request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
                "newName": new_name
            }),
        )
    }

    pub fn request_prepare_rename(&mut self, uri: &str, line: u32, character: u32) -> Value {
        self.send_request(
            "textDocument/prepareRename",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        )
    }

    pub fn request_definition(&mut self, uri: &str, line: u32, character: u32) -> Value {
        self.send_request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        )
    }

    pub fn collect_diagnostics(&mut self) -> Vec<PublishedDiagnostics> {
        let mut result = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);

        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match self
                .messages
                .recv_timeout(remaining.min(Duration::from_millis(500)))
            {
                Ok(msg) if msg.get("method") == Some(&json!("textDocument/publishDiagnostics")) => {
                    let params = &msg["params"];
                    let uri = params["uri"].as_str().unwrap_or_default().to_owned();
                    let diagnostics = params["diagnostics"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .map(|d| DiagnosticInfo {
                                    message: d["message"].as_str().unwrap_or_default().to_owned(),
                                    line: d["range"]["start"]["line"].as_u64().unwrap_or(0),
                                    character: d["range"]["start"]["character"]
                                        .as_u64()
                                        .unwrap_or(0),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    result.push(PublishedDiagnostics { uri, diagnostics });
                }
                Ok(_) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        result
    }

    pub fn shutdown(&mut self) {
        let _ = self.send_request("shutdown", json!(null));
        self.send_notification("exit", json!(null));
        let _ = self.process.wait();
    }

    fn send_request(&mut self, method: &str, params: Value) -> Value {
        self.request_id += 1;
        let msg = json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
            "params": params
        });
        self.write_message(&msg);
        self.read_response(self.request_id)
    }

    fn send_notification(&mut self, method: &str, params: Value) {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.write_message(&msg);
    }

    fn write_message(&mut self, msg: &Value) {
        let body = serde_json::to_string(msg).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        self.stdin.flush().unwrap();
    }

    fn read_response(&mut self, expected_id: i64) -> Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                panic!("timeout waiting for response id={expected_id}");
            }
            match self
                .messages
                .recv_timeout(remaining.min(Duration::from_millis(500)))
            {
                Ok(msg) if msg.get("id") == Some(&json!(expected_id)) => return msg,
                Ok(_) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    panic!("server disconnected while waiting for response id={expected_id}");
                }
            }
        }
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

fn read_header(reader: &mut BufReader<std::process::ChildStdout>) -> Option<usize> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return None;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length: ") {
            content_length = value.parse().ok();
        }
    }
    content_length
}
