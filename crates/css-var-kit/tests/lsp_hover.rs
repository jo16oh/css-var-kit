mod common;

use std::path::Path;

use common::FIXTURES;
use common::lsp_client::LspClient;

// Fixture: tests/fixtures/hover-color/
//
// tokens.css (0-indexed lines):
//   0: :root {
//   1:   --brand: #ff0000;
//   2:   --size: 16px;
//   3:   --primary: var(--brand);
//   4: }
//
// components/app.css (0-indexed lines):
//   0: .x { color: var(--brand); }       <- --brand at col 18
//   1: .y { background: var(--primary); } <- --primary at col 23
//   2: .z { padding: var(--size); }      <- --size at col 20
//   3: .m { color: var(--missing); }     <- --missing at col 18

fn fixture_dir() -> std::path::PathBuf {
    Path::new(FIXTURES).join("hover-color")
}

fn hover_text(value: &serde_json::Value) -> &str {
    value["result"]["contents"]["value"]
        .as_str()
        .unwrap_or_else(|| panic!("expected hover.result.contents.value: {value}"))
}

#[test]
fn hover_on_color_var_returns_swatch_and_value() {
    let dir = fixture_dir();
    let mut client = LspClient::spawn(&dir);
    client.initialize();

    let uri = client.file_uri("components/app.css");
    let text = std::fs::read_to_string(dir.join("components/app.css")).unwrap();
    client.open_document(&uri, &text);

    let response = client.request_hover(&uri, 0, 18);
    let value = hover_text(&response);
    assert!(
        value.contains("data:image/svg+xml;base64,"),
        "swatch missing: {value}"
    );
    assert!(
        value.contains("`#ff0000`"),
        "expected resolved value: {value}"
    );

    client.shutdown();
}

#[test]
fn hover_on_non_color_var_returns_value_only() {
    let dir = fixture_dir();
    let mut client = LspClient::spawn(&dir);
    client.initialize();

    let uri = client.file_uri("components/app.css");
    let text = std::fs::read_to_string(dir.join("components/app.css")).unwrap();
    client.open_document(&uri, &text);

    let response = client.request_hover(&uri, 2, 20);
    let value = hover_text(&response);
    assert!(value.contains("`16px`"), "expected resolved value: {value}");
    assert!(
        !value.contains("data:image/svg"),
        "swatch should be absent: {value}"
    );

    client.shutdown();
}

#[test]
fn hover_on_undefined_var_returns_null() {
    let dir = fixture_dir();
    let mut client = LspClient::spawn(&dir);
    client.initialize();

    let uri = client.file_uri("components/app.css");
    let text = std::fs::read_to_string(dir.join("components/app.css")).unwrap();
    client.open_document(&uri, &text);

    let response = client.request_hover(&uri, 3, 18);
    assert!(
        response["result"].is_null(),
        "expected null result: {response}"
    );

    client.shutdown();
}

#[test]
fn hover_resolves_var_defined_in_other_file() {
    let dir = fixture_dir();
    let mut client = LspClient::spawn(&dir);
    client.initialize();

    let uri = client.file_uri("components/app.css");
    let text = std::fs::read_to_string(dir.join("components/app.css")).unwrap();
    client.open_document(&uri, &text);

    let response = client.request_hover(&uri, 1, 23);
    let value = hover_text(&response);
    assert!(
        value.contains("`#ff0000`"),
        "expected chained resolution to brand color: {value}"
    );

    client.shutdown();
}

#[test]
fn hover_lists_multiple_definitions_across_files() {
    let dir = fixture_dir();
    let mut client = LspClient::spawn(&dir);
    client.initialize();

    let app_uri = client.file_uri("components/app.css");
    let app_text = std::fs::read_to_string(dir.join("components/app.css")).unwrap();
    client.open_document(&app_uri, &app_text);

    let tokens_uri = client.file_uri("tokens.css");
    let tokens_text = std::fs::read_to_string(dir.join("tokens.css")).unwrap();
    let updated = format!("{tokens_text}\n.dark {{ --brand: #0000ff; }}\n");
    client.open_document(&tokens_uri, &tokens_text);
    client.change_document(&tokens_uri, 2, &updated);

    let response = client.request_hover(&app_uri, 0, 18);
    let value = hover_text(&response);
    let entry_count = value.lines().filter(|l| !l.is_empty()).count();
    assert_eq!(entry_count, 2, "expected 2 entries: {value}");
    let swatch_count = value.matches("data:image/svg+xml;base64,").count();
    assert_eq!(swatch_count, 2, "expected 2 swatches: {value}");
    assert!(value.contains("tokens.css:2"), "missing first def: {value}");
    assert!(
        value.contains("tokens.css:7"),
        "missing second def: {value}"
    );

    client.shutdown();
}

#[test]
fn hover_reflects_changes_to_definition_in_other_file() {
    let dir = fixture_dir();
    let mut client = LspClient::spawn(&dir);
    client.initialize();

    let app_uri = client.file_uri("components/app.css");
    let app_text = std::fs::read_to_string(dir.join("components/app.css")).unwrap();
    client.open_document(&app_uri, &app_text);

    let tokens_uri = client.file_uri("tokens.css");
    let tokens_text = std::fs::read_to_string(dir.join("tokens.css")).unwrap();
    client.open_document(&tokens_uri, &tokens_text);

    let updated = tokens_text.replace("#ff0000", "#00ff00");
    client.change_document(&tokens_uri, 2, &updated);

    let response = client.request_hover(&app_uri, 0, 18);
    let value = hover_text(&response);
    assert!(
        value.contains("`#00ff00`"),
        "expected updated value after cross-file edit: {value}"
    );

    client.shutdown();
}
