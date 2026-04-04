mod common;

use std::fs;
use std::path::Path;

use common::lsp_client::LspClient;

use crate::common::copy_fixture_to_tempdir;

#[test]
fn publishes_diagnostics_on_open() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);

    client.initialize();

    let uri = client.file_uri("components/button.css");
    let text = std::fs::read_to_string(fixture_dir.join("components/button.css")).unwrap();
    client.open_document(&uri, &text);

    let diagnostics = client.collect_diagnostics();
    client.shutdown();

    let button_diagnostics: Vec<_> = diagnostics
        .iter()
        .filter(|p| p.uri.ends_with("components/button.css"))
        .flat_map(|p| &p.diagnostics)
        .collect();

    assert!(
        !button_diagnostics.is_empty(),
        "expected diagnostics for button.css, got none"
    );

    let messages: Vec<&str> = button_diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("--spacing-md")),
        "expected diagnostic for --spacing-md, got: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("--border-color")),
        "expected diagnostic for --border-color, got: {messages:?}"
    );
}

#[test]
fn updates_diagnostics_on_background_file_change_via_did_change() {
    let tmp = copy_fixture_to_tempdir("default");
    let mut client = LspClient::spawn(tmp.path());
    client.initialize();

    let button_uri = client.file_uri("components/button.css");
    let button_text = fs::read_to_string(tmp.path().join("components/button.css")).unwrap();
    client.open_document(&button_uri, &button_text);

    let diagnostics = client.collect_diagnostics();
    let messages = collect_messages_for(&diagnostics, "components/button.css");
    assert!(
        messages.iter().any(|m| m.contains("--spacing-md")),
        "expected --spacing-md diagnostic before file change, got: {messages:?}"
    );

    // Simulate background file change: add --spacing-md to variables.css on disk
    fs::write(
        tmp.path().join("variables.css"),
        ":root {\n    --primary-color: #3490dc;\n    --secondary-color: #ffed4a;\n    --font-size-base: 16px;\n    --spacing-md: 1rem;\n}\n",
    )
    .unwrap();

    // Trigger re-analysis via didChange (editor re-sending the same content)
    client.change_document(&button_uri, 2, &button_text);

    let diagnostics = client.collect_diagnostics();
    let messages = collect_messages_for(&diagnostics, "components/button.css");
    assert!(
        !messages.iter().any(|m| m.contains("--spacing-md")),
        "--spacing-md should be resolved after variables.css was updated, got: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("--border-color")),
        "--border-color should still be unresolved, got: {messages:?}"
    );

    client.shutdown();
}

#[test]
fn updates_diagnostics_on_background_file_change_via_watched_files() {
    let tmp = copy_fixture_to_tempdir("default");
    let mut client = LspClient::spawn(tmp.path());
    client.initialize();

    let button_uri = client.file_uri("components/button.css");
    let button_text = fs::read_to_string(tmp.path().join("components/button.css")).unwrap();
    client.open_document(&button_uri, &button_text);

    let diagnostics = client.collect_diagnostics();
    let messages = collect_messages_for(&diagnostics, "components/button.css");
    assert!(
        messages.iter().any(|m| m.contains("--spacing-md")),
        "expected --spacing-md diagnostic before file change, got: {messages:?}"
    );

    // Notify via workspace/didChangeWatchedFiles (as an editor would for external changes)
    fs::write(
        tmp.path().join("variables.css"),
        ":root {\n    --primary-color: #3490dc;\n    --secondary-color: #ffed4a;\n    --font-size-base: 16px;\n    --spacing-md: 1rem;\n}\n",
    )
    .unwrap();
    let variables_uri = client.file_uri("variables.css");
    client.notify_watched_files_changed(&[&variables_uri]);

    let diagnostics = client.collect_diagnostics();
    let messages = collect_messages_for(&diagnostics, "components/button.css");
    assert!(
        !messages.iter().any(|m| m.contains("--spacing-md")),
        "--spacing-md should be resolved after variables.css was updated, got: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("--border-color")),
        "--border-color should still be unresolved, got: {messages:?}"
    );

    client.shutdown();
}

/// Server-side file watcher: diagnostics update without any client notification,
/// triggered purely by the notify crate detecting disk changes.
#[test]
fn updates_diagnostics_on_background_file_change_via_server_watcher() {
    let tmp_dir = copy_fixture_to_tempdir("default");
    let mut client = LspClient::spawn(tmp_dir.path());
    client.initialize();

    let button_uri = client.file_uri("components/button.css");
    let button_text = fs::read_to_string(tmp_dir.path().join("components/button.css")).unwrap();
    client.open_document(&button_uri, &button_text);

    let diagnostics = client.collect_diagnostics();
    let messages = collect_messages_for(&diagnostics, "components/button.css");
    assert!(
        messages.iter().any(|m| m.contains("--spacing-md")),
        "expected --spacing-md diagnostic before file change, got: {messages:?}"
    );

    // Modify variables.css on disk without sending any LSP notification.
    // The server-side file watcher (notify crate) should detect this and re-publish.
    fs::write(
        tmp_dir.path().join("variables.css"),
        ":root {\n    --primary-color: #3490dc;\n    --secondary-color: #ffed4a;\n    --font-size-base: 16px;\n    --spacing-md: 1rem;\n}\n",
    )
    .unwrap();

    let diagnostics = client.collect_diagnostics();
    let messages = collect_messages_for(&diagnostics, "components/button.css");
    assert!(
        !messages.iter().any(|m| m.contains("--spacing-md")),
        "--spacing-md should be resolved after variables.css was updated on disk, got: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("--border-color")),
        "--border-color should still be unresolved, got: {messages:?}"
    );

    client.shutdown();
}

fn collect_messages_for<'a>(
    diagnostics: &'a [common::lsp_client::PublishedDiagnostics],
    suffix: &str,
) -> Vec<&'a str> {
    diagnostics
        .iter()
        .filter(|p| p.uri.ends_with(suffix))
        .flat_map(|p| &p.diagnostics)
        .map(|d| d.message.as_str())
        .collect()
}
