mod common;

use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

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

    // Simulate editing variables.css: add --spacing-md via didChange
    let variables_uri = client.file_uri("variables.css");
    let new_variables_text = ":root {\n    --primary-color: #3490dc;\n    --secondary-color: #ffed4a;\n    --font-size-base: 16px;\n    --spacing-md: 1rem;\n}\n";
    client.change_document(&variables_uri, 2, new_variables_text);

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
    let mut client = LspClient::spawn_with_args(tmp.path(), &["--log"]);
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

    sleep(Duration::from_millis(1100));

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

#[test]
fn writes_log_file_when_configured() {
    let tmp = copy_fixture_to_tempdir("default");
    let log_file = tmp.path().join("cvk-lsp.log");

    // Add logFile to cvk.json
    fs::write(
        tmp.path().join("cvk.json"),
        format!(r#"{{ "lsp": {{ "logFile": "{}" }} }}"#, log_file.display()),
    )
    .unwrap();

    let mut client = LspClient::spawn_with_args(tmp.path(), &["--log"]);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    let text = fs::read_to_string(tmp.path().join("components/button.css")).unwrap();
    client.open_document(&uri, &text);

    let _ = client.collect_diagnostics();
    client.shutdown();

    let log_content = fs::read_to_string(&log_file).expect("log file should exist");
    assert!(
        log_content.contains("initialized:"),
        "log should contain initialized message, got: {log_content}"
    );
    assert!(
        log_content.contains("textDocument/didOpen"),
        "log should contain didOpen message, got: {log_content}"
    );
    assert!(
        log_content.contains("publishDiagnostics:"),
        "log should contain publishDiagnostics message, got: {log_content}"
    );
    assert!(
        log_content.contains("shutdown"),
        "log should contain shutdown message, got: {log_content}"
    );
}

#[test]
fn completes_color_variables_for_color_property() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    let text = fs::read_to_string(fixture_dir.join("components/button.css")).unwrap();
    client.open_document(&uri, &text);
    let _ = client.collect_diagnostics();

    // line 1: "    color: var(--primary-color);" — cursor inside the value
    let response = client.request_completion(&uri, 1, 12);
    client.shutdown();

    let items = response["result"]
        .as_array()
        .expect("expected completion array");

    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

    assert!(
        labels.contains(&"--primary-color"),
        "color variable should be suggested for color property, got: {labels:?}"
    );
    assert!(
        labels.contains(&"--secondary-color"),
        "color variable should be suggested for color property, got: {labels:?}"
    );
    assert!(
        !labels.contains(&"--font-size-base"),
        "length variable should NOT be suggested for color property, got: {labels:?}"
    );
}

#[test]
fn completes_length_variables_for_font_size_property() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    let text = fs::read_to_string(fixture_dir.join("components/button.css")).unwrap();
    client.open_document(&uri, &text);
    let _ = client.collect_diagnostics();

    // line 3: "    font-size: var(--font-size-base);" — cursor inside the value
    let response = client.request_completion(&uri, 3, 16);
    client.shutdown();

    let items = response["result"]
        .as_array()
        .expect("expected completion array");

    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

    assert!(
        labels.contains(&"--font-size-base"),
        "length variable should be suggested for font-size, got: {labels:?}"
    );
    assert!(
        !labels.contains(&"--primary-color"),
        "color variable should NOT be suggested for font-size, got: {labels:?}"
    );
}

#[test]
fn completion_text_edit_replaces_typed_prefix() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    // "    color: --" — user typed "--" outside var()
    let text = ".test {\n    color: --\n}\n";
    client.open_document(&uri, text);
    let _ = client.collect_diagnostics();

    // cursor after "--" (col 13)
    let response = client.request_completion(&uri, 1, 13);
    client.shutdown();

    let items = response["result"]
        .as_array()
        .expect("expected completion array");

    let primary = items
        .iter()
        .find(|i| i["label"].as_str() == Some("--primary-color"))
        .expect("--primary-color not found in completions");

    let text_edit = &primary["textEdit"];
    assert_eq!(
        text_edit["newText"].as_str(),
        Some("var(--primary-color)"),
        "newText should wrap variable in var()"
    );
    // replace range should cover the typed "--" (col 11..13)
    assert_eq!(text_edit["range"]["start"]["character"], 11);
    assert_eq!(text_edit["range"]["end"]["character"], 13);
    assert_eq!(
        primary["detail"].as_str(),
        Some("blue"),
        "detail should show the last variable value"
    );
}

#[test]
fn completion_filters_by_compound_value_prefix() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    // border already has width (1px) and style (solid), only color slot remains
    let text = ".test {\n    border: 1px solid -\n}\n";
    client.open_document(&uri, text);
    let _ = client.collect_diagnostics();

    // cursor after "-" on "    border: 1px solid -"
    let response = client.request_completion(&uri, 1, 23);
    client.shutdown();

    let items = response["result"]
        .as_array()
        .expect("expected completion array");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

    assert!(
        labels.contains(&"--primary-color"),
        "color variable should be suggested for border color slot, got: {labels:?}"
    );
    assert!(
        !labels.contains(&"--font-size-base"),
        "length variable should NOT be suggested when border already has width, got: {labels:?}"
    );
}

#[test]
fn completion_considers_value_after_cursor() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    // "border: 1px -- red;" — width and color already present, cursor at "--"
    let text = ".test {\n    border: 1px -- red;\n}\n";
    client.open_document(&uri, text);
    let _ = client.collect_diagnostics();

    // cursor after "--" on "    border: 1px --"
    //     b o r d e r :   1 p x   - -
    // 0   4             9  11      13
    let response = client.request_completion(&uri, 1, 18);
    client.shutdown();

    let items = response["result"]
        .as_array()
        .expect("expected completion array");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

    // "1px" fills width, "red" fills color — only style-compatible vars should remain
    assert!(
        !labels.contains(&"--primary-color"),
        "color variable should NOT be suggested when color is already present, got: {labels:?}"
    );
    assert!(
        !labels.contains(&"--font-size-base"),
        "length variable should NOT be suggested when width is already present, got: {labels:?}"
    );
}

#[test]
fn completion_filters_inside_function_argument() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    // translate() accepts lengths, not colors
    let text = ".test {\n    transform: translate(-\n}\n";
    client.open_document(&uri, text);
    let _ = client.collect_diagnostics();

    // cursor after "-" on "    transform: translate(-"
    let response = client.request_completion(&uri, 1, 26);
    client.shutdown();

    let items = response["result"]
        .as_array()
        .expect("expected completion array");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

    assert!(
        labels.contains(&"--font-size-base"),
        "length variable should be suggested inside translate(), got: {labels:?}"
    );
    assert!(
        !labels.contains(&"--primary-color"),
        "color variable should NOT be suggested inside translate(), got: {labels:?}"
    );
}

#[test]
fn completion_inside_var_inserts_variable_name_only() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    let text = ".test {\n    color: var(-\n}\n";
    client.open_document(&uri, text);
    let _ = client.collect_diagnostics();

    // cursor after "-" on "    color: var(-"
    let response = client.request_completion(&uri, 1, 16);
    client.shutdown();

    let items = response["result"]
        .as_array()
        .expect("expected completion array");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

    assert!(
        labels.contains(&"--primary-color"),
        "color variable should be suggested inside var(), got: {labels:?}"
    );
    assert!(
        !labels.contains(&"--font-size-base"),
        "length variable should NOT be suggested for color inside var(), got: {labels:?}"
    );

    let primary = items
        .iter()
        .find(|i| i["label"].as_str() == Some("--primary-color"))
        .unwrap();
    let text_edit = &primary["textEdit"];
    assert_eq!(
        text_edit["newText"].as_str(),
        Some("--primary-color"),
        "newText inside var() should be variable name only, not wrapped in var()"
    );
    // "    color: var(-" → var( ends at col 15, "-" at col 15, cursor at col 16
    // replace range should cover the typed "-" (col 15..16)
    assert_eq!(text_edit["range"]["start"]["character"], 15);
    assert_eq!(text_edit["range"]["end"]["character"], 16);
}

#[test]
fn completion_not_triggered_outside_property_value() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    let text = fs::read_to_string(fixture_dir.join("components/button.css")).unwrap();
    client.open_document(&uri, &text);
    let _ = client.collect_diagnostics();

    // Request completion at line 0 (.button {), which is a selector, not a value
    let response = client.request_completion(&uri, 0, 5);
    client.shutdown();

    let result = &response["result"];
    assert!(
        result.is_null(),
        "expected null result outside property value, got: {result}"
    );
}

#[test]
fn goto_definition_jumps_to_variable_declaration() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let button_uri = client.file_uri("components/button.css");
    let text = fs::read_to_string(fixture_dir.join("components/button.css")).unwrap();
    client.open_document(&button_uri, &text);
    let _ = client.collect_diagnostics();

    // line 1: "    color: var(--primary-color);"
    // cursor on "--primary-color" (col 19)
    let response = client.request_definition(&button_uri, 1, 19);
    client.shutdown();

    let result = response["result"]
        .as_array()
        .expect("expected definition array");

    // --primary-color is defined in :root (line 1) and .dark (line 7)
    assert_eq!(result.len(), 2, "expected 2 definitions, got: {result:?}");

    for loc in result {
        let uri = loc["uri"].as_str().unwrap();
        assert!(
            uri.ends_with("/variables.css"),
            "expected URI ending with /variables.css, got: {uri}"
        );
        assert_eq!(loc["range"]["start"]["character"], 4);
        // 4 + len("--primary-color") = 19
        assert_eq!(loc["range"]["end"]["character"], 19);
    }

    let lines: Vec<u64> = result
        .iter()
        .map(|loc| loc["range"]["start"]["line"].as_u64().unwrap())
        .collect();
    assert!(lines.contains(&1), "expected definition at line 1 (:root)");
    assert!(lines.contains(&7), "expected definition at line 7 (.dark)");
}

#[test]
fn goto_definition_returns_null_for_undefined_variable() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    let text = ".test {\n    color: var(--nonexistent);\n}\n";
    client.open_document(&uri, text);
    let _ = client.collect_diagnostics();

    // cursor on "--nonexistent" (col 21)
    let response = client.request_definition(&uri, 1, 21);
    client.shutdown();

    let result = &response["result"];
    assert!(
        result.is_null(),
        "expected null for undefined variable, got: {result}"
    );
}

#[test]
fn goto_definition_returns_null_outside_variable() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    let text = fs::read_to_string(fixture_dir.join("components/button.css")).unwrap();
    client.open_document(&uri, &text);
    let _ = client.collect_diagnostics();

    // line 0: ".button {" — cursor on selector, not a variable
    let response = client.request_definition(&uri, 0, 3);
    client.shutdown();

    let result = &response["result"];
    assert!(
        result.is_null(),
        "expected null outside variable, got: {result}"
    );
}

#[test]
fn rename_symbol_renames_definitions_and_usages() {
    let tmp = copy_fixture_to_tempdir("default");
    let workspace = tmp.path();
    let mut client = LspClient::spawn(workspace);
    client.initialize();

    let button_uri = client.file_uri("components/button.css");
    let button_text = fs::read_to_string(workspace.join("components/button.css")).unwrap();
    client.open_document(&button_uri, &button_text);

    let vars_uri = client.file_uri("variables.css");
    let vars_text = fs::read_to_string(workspace.join("variables.css")).unwrap();
    client.open_document(&vars_uri, &vars_text);
    let _ = client.collect_diagnostics();

    // cursor on "--primary-color" in button.css line 1: "    color: var(--primary-color);"
    let response = client.request_rename(&button_uri, 1, 19, "--brand-color");
    client.shutdown();

    let result = &response["result"];
    assert!(!result.is_null(), "expected workspace edit, got null");

    let changes = result["changes"].as_object().expect("expected changes map");

    // definitions in variables.css (lines 1 and 7)
    let vars_edits: Vec<&serde_json::Value> = changes
        .iter()
        .filter(|(uri, _)| uri.ends_with("/variables.css"))
        .flat_map(|(_, edits)| edits.as_array().unwrap())
        .collect();
    assert_eq!(
        vars_edits.len(),
        2,
        "expected 2 definition edits: {vars_edits:?}"
    );

    // usage in button.css (line 1)
    let button_edits: Vec<&serde_json::Value> = changes
        .iter()
        .filter(|(uri, _)| uri.ends_with("/button.css"))
        .flat_map(|(_, edits)| edits.as_array().unwrap())
        .collect();
    assert_eq!(
        button_edits.len(),
        1,
        "expected 1 usage edit: {button_edits:?}"
    );

    for edit in vars_edits.iter().chain(button_edits.iter()) {
        assert_eq!(edit["newText"].as_str().unwrap(), "--brand-color");
    }
}

#[test]
fn rename_symbol_auto_prepends_dashes() {
    let tmp = copy_fixture_to_tempdir("default");
    let workspace = tmp.path();
    let mut client = LspClient::spawn(workspace);
    client.initialize();

    let vars_uri = client.file_uri("variables.css");
    let vars_text = fs::read_to_string(workspace.join("variables.css")).unwrap();
    client.open_document(&vars_uri, &vars_text);
    let _ = client.collect_diagnostics();

    // cursor on "--secondary-color" definition at line 2
    let response = client.request_rename(&vars_uri, 2, 6, "accent-color");
    client.shutdown();

    let result = &response["result"];
    let changes = result["changes"].as_object().expect("expected changes map");
    let edits: Vec<&serde_json::Value> = changes
        .values()
        .flat_map(|edits| edits.as_array().unwrap())
        .collect();

    for edit in &edits {
        assert_eq!(
            edit["newText"].as_str().unwrap(),
            "--accent-color",
            "should auto-prepend -- when user omits it"
        );
    }
}

#[test]
fn rename_returns_null_outside_variable() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    let text = fs::read_to_string(fixture_dir.join("components/button.css")).unwrap();
    client.open_document(&uri, &text);
    let _ = client.collect_diagnostics();

    // cursor on ".button" selector, not a variable
    let response = client.request_rename(&uri, 0, 3, "--new-name");
    client.shutdown();

    let result = &response["result"];
    assert!(
        result.is_null(),
        "expected null outside variable, got: {result}"
    );
}

#[test]
fn prepare_rename_succeeds_on_variable() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    let text = fs::read_to_string(fixture_dir.join("components/button.css")).unwrap();
    client.open_document(&uri, &text);
    let _ = client.collect_diagnostics();

    // line 1: "    color: var(--primary-color);"
    // cursor on "--primary-color" in var() (col 19)
    let response = client.request_prepare_rename(&uri, 1, 19);
    client.shutdown();

    let result = &response["result"];
    assert!(
        !result.is_null(),
        "expected prepare rename result, got null"
    );

    // range should cover exactly "--primary-color" (cols 15..30)
    assert_eq!(result["range"]["start"]["line"], 1);
    assert_eq!(result["range"]["start"]["character"], 15);
    assert_eq!(result["range"]["end"]["line"], 1);
    assert_eq!(result["range"]["end"]["character"], 30);
    assert_eq!(result["placeholder"], "primary-color");
}

#[test]
fn prepare_rename_returns_null_outside_variable() {
    let fixture_dir = Path::new(common::FIXTURES).join("default");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("components/button.css");
    let text = fs::read_to_string(fixture_dir.join("components/button.css")).unwrap();
    client.open_document(&uri, &text);
    let _ = client.collect_diagnostics();

    // cursor on selector
    let response = client.request_prepare_rename(&uri, 0, 3);
    client.shutdown();

    let result = &response["result"];
    assert!(
        result.is_null(),
        "expected null outside variable, got: {result}"
    );
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
