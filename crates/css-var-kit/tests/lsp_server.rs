mod common;
mod lsp;

use std::path::Path;

use lsp::LspClient;

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
