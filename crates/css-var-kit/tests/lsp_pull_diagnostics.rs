mod common;

use std::path::Path;

use crate::common::copy_fixture_to_tempdir;
use crate::common::lsp_client::LspClient;

// Helper: collect diagnostic messages from a textDocument/diagnostic Full response.
fn document_diagnostic_messages(response: &serde_json::Value) -> Vec<&str> {
    response["result"]["items"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|d| d["message"].as_str()).collect())
        .unwrap_or_default()
}

// Helper: collect (uri, messages) pairs from a workspace/diagnostic response.
fn workspace_diagnostic_messages(response: &serde_json::Value) -> Vec<(String, Vec<String>)> {
    response["result"]["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| {
            let uri = entry["uri"].as_str().unwrap_or_default().to_owned();
            let messages = entry["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|d| d["message"].as_str().map(str::to_owned))
                .collect();
            (uri, messages)
        })
        .collect()
}

#[test]
fn document_diagnostic_returns_diagnostics_without_opening_file() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    // Request diagnostics for button.css without ever calling open_document.
    let button_uri = client.file_uri("components/button.css");
    let response = client.request_document_diagnostic(&button_uri);
    client.shutdown();

    assert_eq!(
        response["result"]["kind"].as_str(),
        Some("full"),
        "expected a Full report, got: {}",
        response["result"]
    );

    let messages = document_diagnostic_messages(&response);
    assert!(
        messages.iter().any(|m| m.contains("--spacing-md")),
        "expected --spacing-md diagnostic, got: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("--border-color")),
        "expected --border-color diagnostic, got: {messages:?}"
    );
}

#[test]
fn document_diagnostic_returns_empty_for_file_without_errors() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    // variables.css only defines variables, no usages of undefined vars.
    let uri = client.file_uri("variables.css");
    let response = client.request_document_diagnostic(&uri);
    client.shutdown();

    assert_eq!(
        response["result"]["kind"].as_str(),
        Some("full"),
        "expected a Full report, got: {}",
        response["result"]
    );

    let messages = document_diagnostic_messages(&response);
    assert!(
        messages.is_empty(),
        "expected no diagnostics for variables.css, got: {messages:?}"
    );
}

#[test]
fn workspace_diagnostic_covers_all_files_without_opening_any() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    // No files opened — workspace/diagnostic must still return results for all
    // workspace files loaded from disk at startup.
    let response = client.request_workspace_diagnostic();
    client.shutdown();

    let items = workspace_diagnostic_messages(&response);
    assert!(
        !items.is_empty(),
        "expected workspace items, got empty result"
    );

    let button_messages: Vec<&str> = items
        .iter()
        .filter(|(uri, _)| uri.ends_with("components/button.css"))
        .flat_map(|(_, msgs)| msgs.iter().map(String::as_str))
        .collect();
    assert!(
        button_messages.iter().any(|m| m.contains("--spacing-md")),
        "expected --spacing-md in button.css workspace diagnostics, got: {button_messages:?}"
    );

    let card_messages: Vec<&str> = items
        .iter()
        .filter(|(uri, _)| uri.ends_with("components/card.css"))
        .flat_map(|(_, msgs)| msgs.iter().map(String::as_str))
        .collect();
    assert!(
        card_messages.iter().any(|m| m.contains("--radius-lg")),
        "expected --radius-lg in card.css workspace diagnostics, got: {card_messages:?}"
    );
}

#[test]
fn workspace_diagnostic_clears_on_definition_added() {
    let tmp = copy_fixture_to_tempdir("default");
    let mut client = LspClient::spawn(tmp.path());
    client.initialize();

    let response = client.request_workspace_diagnostic();
    let items = workspace_diagnostic_messages(&response);
    let button_messages: Vec<&str> = items
        .iter()
        .filter(|(uri, _)| uri.ends_with("components/button.css"))
        .flat_map(|(_, msgs)| msgs.iter().map(String::as_str))
        .collect();
    assert!(
        button_messages.iter().any(|m| m.contains("--spacing-md")),
        "--spacing-md should be undefined before fix, got: {button_messages:?}"
    );

    // Add --spacing-md to variables.css via didChange.
    let vars_uri = client.file_uri("variables.css");
    client.change_document(
        &vars_uri,
        2,
        ":root {\n  --primary-color: #3490dc;\n  --secondary-color: #ffed4a;\n  --font-size-base: 16px;\n  --spacing-md: 1rem;\n}\n.dark {\n  --primary-color: blue;\n}\n",
    );
    // Consume the push diagnostics triggered by the change.
    let _ = client.collect_diagnostics();

    let response = client.request_workspace_diagnostic();
    client.shutdown();

    let items = workspace_diagnostic_messages(&response);
    let button_messages: Vec<&str> = items
        .iter()
        .filter(|(uri, _)| uri.ends_with("components/button.css"))
        .flat_map(|(_, msgs)| msgs.iter().map(String::as_str))
        .collect();
    assert!(
        !button_messages.iter().any(|m| m.contains("--spacing-md")),
        "--spacing-md should be resolved after definition added, got: {button_messages:?}"
    );
}
