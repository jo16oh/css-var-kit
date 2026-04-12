mod common;

use std::path::Path;

use common::lsp_client::LspClient;
use common::{FIXTURES, copy_fixture_to_tempdir};

// Fixture: tests/fixtures/html-like-lsp/
//
// Component.vue (0-indexed lines):
//   0: <template>
//   1:   <div class="box">Hello</div>
//   2: </template>
//   3:
//   4: <style>
//   5: .box {
//   6:   color: var(--color-primary);
//   7:   margin: var(--undefined-spacing);
//   8: }
//   9: </style>
//
// tokens.css (0-indexed lines):
//   0: :root {
//   1:   --color-primary: #3490dc;   ← col 2
//   2:   --spacing-md: 16px;
//   3: }
//
// Tokens.vue (0-indexed lines):
//   0: <style>
//   1: :root {
//   2:   --vue-color: #ff6b6b;       ← col 2
//   3: }
//   4: </style>

fn fixture_dir() -> std::path::PathBuf {
    Path::new(FIXTURES).join("html-like-lsp")
}

// ── Diagnostics ─────────────────────────────────────────────────────────────

/// Vue ファイルを開いたとき、未定義変数のエラーが発行される。
#[test]
fn publishes_diagnostics_on_open_vue_file() {
    let dir = fixture_dir();
    let mut client = LspClient::spawn(&dir);
    client.initialize();

    let uri = client.file_uri("Component.vue");
    let text = std::fs::read_to_string(dir.join("Component.vue")).unwrap();
    client.open_document(&uri, &text);

    let diagnostics = client.collect_diagnostics();
    client.shutdown();

    let messages: Vec<&str> = diagnostics
        .iter()
        .filter(|p| p.uri.ends_with("Component.vue"))
        .flat_map(|p| &p.diagnostics)
        .map(|d| d.message.as_str())
        .collect();

    assert!(
        messages.iter().any(|m| m.contains("--undefined-spacing")),
        "expected diagnostic for --undefined-spacing, got: {messages:?}"
    );
}

/// 診断の行番号が Vue ファイル全体での絶対行番号になっている。
/// `--undefined-spacing` は Component.vue の 7 行目（0-indexed）にある。
#[test]
fn diagnostic_line_number_is_absolute_in_vue_file() {
    let dir = fixture_dir();
    let mut client = LspClient::spawn(&dir);
    client.initialize();

    let uri = client.file_uri("Component.vue");
    let text = std::fs::read_to_string(dir.join("Component.vue")).unwrap();
    client.open_document(&uri, &text);

    let published = client.collect_diagnostics();
    client.shutdown();

    let diag = published
        .iter()
        .filter(|p| p.uri.ends_with("Component.vue"))
        .flat_map(|p| &p.diagnostics)
        .find(|d| d.message.contains("--undefined-spacing"))
        .expect("diagnostic for --undefined-spacing not found");

    // `  margin: var(--undefined-spacing);` は 7 行目（0-indexed）
    assert_eq!(
        diag.line, 7,
        "diagnostic should point to line 7, got: {}",
        diag.line
    );
    // `  margin: var(` = 14 文字なので、--undefined-spacing は col 14 から始まる
    assert_eq!(
        diag.character, 14,
        "diagnostic should point to col 14, got: {}",
        diag.character
    );
}

// ── Completion ───────────────────────────────────────────────────────────────

/// Vue の `<style>` ブロック内でカーソルを `var(--` の後に置いたとき、補完が返る。
/// line 6: `  color: var(--color-primary);` の col 14 (--color-primary の先頭) でリクエスト。
#[test]
fn completion_in_vue_style_block() {
    let dir = fixture_dir();
    let mut client = LspClient::spawn(&dir);
    client.initialize();

    let uri = client.file_uri("Component.vue");
    let text = std::fs::read_to_string(dir.join("Component.vue")).unwrap();
    client.open_document(&uri, &text);
    let _ = client.collect_diagnostics();

    // line 6: `  color: var(--color-primary);`, cursor at col 14 (inside var)
    let response = client.request_completion(&uri, 6, 14);
    client.shutdown();

    let items = response["result"]
        .as_array()
        .expect("expected completion array");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

    // color プロパティには color 型変数のみ提案される
    assert!(
        labels.contains(&"--color-primary"),
        "--color-primary should be suggested, got: {labels:?}"
    );
    assert!(
        !labels.contains(&"--spacing-md"),
        "--spacing-md (length) should NOT be suggested for color property, got: {labels:?}"
    );
}

/// デフォルトでは Vue ファイル内の変数定義は lookupFiles の対象外なので補完候補に現れない。
/// `lookupFiles` に `**/*.vue` を追加すると候補に出るようになる（opt-in）。
#[test]
fn completion_includes_vue_defined_variables_when_opted_in() {
    let tmp = copy_fixture_to_tempdir("html-like-lsp");
    // cvk.json に lookupFiles: ["**/*.css", "**/*.vue"] を書き込む
    std::fs::write(
        tmp.path().join("cvk.json"),
        r#"{"lookupFiles":["**/*.css","**/*.vue"]}"#,
    )
    .unwrap();

    let mut client = LspClient::spawn(tmp.path());
    client.initialize();

    let uri = client.file_uri("Component.vue");
    let text = std::fs::read_to_string(tmp.path().join("Component.vue")).unwrap();
    client.open_document(&uri, &text);
    let _ = client.collect_diagnostics();

    let response = client.request_completion(&uri, 6, 14);
    client.shutdown();

    let items = response["result"]
        .as_array()
        .expect("expected completion array");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

    assert!(
        labels.contains(&"--vue-color"),
        "--vue-color defined in Tokens.vue should be suggested when *.vue is in lookupFiles, got: {labels:?}"
    );
}

/// デフォルト設定では Vue ファイル内の変数定義は lookupFiles 対象外なので補完に出ない。
#[test]
fn completion_excludes_vue_defined_variables_by_default() {
    let dir = fixture_dir();
    let mut client = LspClient::spawn(&dir);
    client.initialize();

    let uri = client.file_uri("Component.vue");
    let text = std::fs::read_to_string(dir.join("Component.vue")).unwrap();
    client.open_document(&uri, &text);
    let _ = client.collect_diagnostics();

    let response = client.request_completion(&uri, 6, 14);
    client.shutdown();

    let items = response["result"]
        .as_array()
        .expect("expected completion array");
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

    assert!(
        !labels.contains(&"--vue-color"),
        "--vue-color should NOT be suggested by default (not in lookupFiles), got: {labels:?}"
    );
}

// ── Go to definition ─────────────────────────────────────────────────────────

/// Vue の `<style>` ブロック内の `var(--color-primary)` でジャンプ定義をリクエストする。
/// `tokens.css` の行 1 にある定義へジャンプするはず。
#[test]
fn goto_definition_from_vue_style_block() {
    let dir = fixture_dir();
    let mut client = LspClient::spawn(&dir);
    client.initialize();

    let uri = client.file_uri("Component.vue");
    let text = std::fs::read_to_string(dir.join("Component.vue")).unwrap();
    client.open_document(&uri, &text);
    let _ = client.collect_diagnostics();

    // line 6: `  color: var(--color-primary);`, cursor on --color-primary (col 14)
    let response = client.request_definition(&uri, 6, 14);
    client.shutdown();

    let result = response["result"]
        .as_array()
        .expect("expected definition array");

    assert_eq!(result.len(), 1, "expected 1 definition, got: {result:?}");

    let loc = &result[0];
    assert!(
        loc["uri"].as_str().unwrap().ends_with("/tokens.css"),
        "expected definition in tokens.css, got: {}",
        loc["uri"]
    );
    // tokens.css line 1: `  --color-primary: #3490dc;` — col 2
    assert_eq!(loc["range"]["start"]["line"], 1);
    assert_eq!(loc["range"]["start"]["character"], 2);
}

/// Tokens.vue で定義された変数への定義ジャンプが Vue ファイル内の絶対行番号を返す。
/// Tokens.vue line 2: `  --vue-color: #ff6b6b;` — col 2
/// `lookupFiles` に `**/*.vue` を追加した場合（opt-in）のみ動作する。
#[test]
fn goto_definition_points_to_correct_line_in_vue_definition_file() {
    let tmp = copy_fixture_to_tempdir("html-like-lsp");
    std::fs::write(
        tmp.path().join("cvk.json"),
        r#"{"lookupFiles":["**/*.css","**/*.vue"]}"#,
    )
    .unwrap();

    let mut client = LspClient::spawn(tmp.path());
    client.initialize();

    // Component.vue で --vue-color が使われるよう上書き（その場でテキストを渡す）
    let uri = client.file_uri("Component.vue");
    let text = "<style>\n.box {\n  color: var(--vue-color);\n}\n</style>\n";
    client.open_document(&uri, text);
    let _ = client.collect_diagnostics();

    // line 2: `  color: var(--vue-color);`, cursor on --vue-color (col 14)
    let response = client.request_definition(&uri, 2, 14);
    client.shutdown();

    let result = response["result"]
        .as_array()
        .expect("expected definition array");

    assert_eq!(result.len(), 1, "expected 1 definition, got: {result:?}");

    let loc = &result[0];
    assert!(
        loc["uri"].as_str().unwrap().ends_with("/Tokens.vue"),
        "expected definition in Tokens.vue, got: {}",
        loc["uri"]
    );
    // Tokens.vue line 2: `  --vue-color: #ff6b6b;` — col 2
    assert_eq!(loc["range"]["start"]["line"], 2);
    assert_eq!(loc["range"]["start"]["character"], 2);
}

// ── Rename ────────────────────────────────────────────────────────────────────

/// Vue の `<style>` ブロック内の変数をリネームすると、Vue ファイルと CSS ファイル
/// 両方のエディットが返る。
#[test]
fn rename_from_vue_style_block_edits_both_files() {
    let tmp = copy_fixture_to_tempdir("html-like-lsp");
    let mut client = LspClient::spawn(tmp.path());
    client.initialize();

    let uri = client.file_uri("Component.vue");
    let text = std::fs::read_to_string(tmp.path().join("Component.vue")).unwrap();
    client.open_document(&uri, &text);

    let tokens_uri = client.file_uri("tokens.css");
    let tokens_text = std::fs::read_to_string(tmp.path().join("tokens.css")).unwrap();
    client.open_document(&tokens_uri, &tokens_text);

    let _ = client.collect_diagnostics();

    // line 6: `  color: var(--color-primary);`, cursor on --color-primary (col 14)
    let response = client.request_rename(&uri, 6, 14, "--brand-color");
    client.shutdown();

    let result = &response["result"];
    assert!(!result.is_null(), "expected workspace edit, got null");

    let changes = result["changes"].as_object().expect("expected changes map");

    let vue_edits: Vec<&serde_json::Value> = changes
        .iter()
        .filter(|(uri, _)| uri.ends_with("/Component.vue"))
        .flat_map(|(_, edits)| edits.as_array().unwrap())
        .collect();
    assert!(
        !vue_edits.is_empty(),
        "expected edits in Component.vue: {changes:?}"
    );

    let css_edits: Vec<&serde_json::Value> = changes
        .iter()
        .filter(|(uri, _)| uri.ends_with("/tokens.css"))
        .flat_map(|(_, edits)| edits.as_array().unwrap())
        .collect();
    assert!(
        !css_edits.is_empty(),
        "expected edits in tokens.css: {changes:?}"
    );

    for edit in vue_edits.iter().chain(css_edits.iter()) {
        assert_eq!(edit["newText"].as_str().unwrap(), "--brand-color");
    }
}

/// Vue ファイル内の変数定義をリネームすると、使用箇所も同時に書き換えられる。
/// `lookupFiles` に `**/*.vue` を追加した場合（opt-in）のみ動作する。
#[test]
fn rename_vue_defined_variable() {
    let tmp = copy_fixture_to_tempdir("html-like-lsp");
    std::fs::write(
        tmp.path().join("cvk.json"),
        r#"{"lookupFiles":["**/*.css","**/*.vue"]}"#,
    )
    .unwrap();

    let mut client = LspClient::spawn(tmp.path());
    client.initialize();

    // Component.vue で --vue-color を使うよう書き換えたテキストを渡す
    let comp_uri = client.file_uri("Component.vue");
    let comp_text = "<style>\n.box {\n  color: var(--vue-color);\n}\n</style>\n";
    client.open_document(&comp_uri, comp_text);

    let tokens_uri = client.file_uri("Tokens.vue");
    let tokens_text = std::fs::read_to_string(tmp.path().join("Tokens.vue")).unwrap();
    client.open_document(&tokens_uri, &tokens_text);

    let _ = client.collect_diagnostics();

    // Tokens.vue line 2: `  --vue-color: #ff6b6b;`, cursor on --vue-color (col 2)
    let response = client.request_rename(&tokens_uri, 2, 4, "--vue-brand");
    client.shutdown();

    let result = &response["result"];
    assert!(!result.is_null(), "expected workspace edit, got null");

    let changes = result["changes"].as_object().expect("expected changes map");

    // Tokens.vue の定義側
    let tokens_edits: Vec<_> = changes
        .iter()
        .filter(|(uri, _)| uri.ends_with("/Tokens.vue"))
        .flat_map(|(_, edits)| edits.as_array().unwrap())
        .collect();
    assert!(
        !tokens_edits.is_empty(),
        "expected edits in Tokens.vue: {changes:?}"
    );

    // Component.vue の使用側（source_cache 経由で参照される）
    let comp_edits: Vec<_> = changes
        .iter()
        .filter(|(uri, _)| uri.ends_with("/Component.vue"))
        .flat_map(|(_, edits)| edits.as_array().unwrap())
        .collect();
    assert!(
        !comp_edits.is_empty(),
        "expected edits in Component.vue: {changes:?}"
    );

    for edit in tokens_edits.iter().chain(comp_edits.iter()) {
        assert_eq!(edit["newText"].as_str().unwrap(), "--vue-brand");
    }
}
