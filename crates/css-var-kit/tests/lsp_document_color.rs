mod common;

use std::path::Path;

use common::lsp_client::LspClient;

#[test]
fn returns_color_info_only_for_var_usages() {
    let fixture_dir = Path::new(common::FIXTURES).join("document-color");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("styles.css");
    let text = std::fs::read_to_string(fixture_dir.join("styles.css")).unwrap();
    client.open_document(&uri, &text);

    let response = client.request_document_color(&uri);
    client.shutdown();

    let result = response.get("result").expect("expected result");
    let infos = result.as_array().expect("result should be array");

    let lines: Vec<u64> = infos
        .iter()
        .map(|info| info["range"]["start"]["line"].as_u64().unwrap())
        .collect();

    assert_eq!(
        infos.len(),
        2,
        "expected 2 color infos (var(--brand) and var(--primary)), got {}: {:?}",
        infos.len(),
        infos
    );

    assert!(
        lines.contains(&7),
        "expected color info on line 7 (var(--brand)), got lines={lines:?}"
    );
    assert!(
        lines.contains(&8),
        "expected color info on line 8 (var(--primary)), got lines={lines:?}"
    );

    for line in &lines {
        assert_ne!(*line, 1, "definition --brand line should not have a swatch");
        assert_ne!(
            *line, 2,
            "definition --primary line should not have a swatch"
        );
        assert_ne!(
            *line, 9,
            "var(--size) is not a color and should not have a swatch"
        );
    }

    for info in infos {
        let red = info["color"]["red"].as_f64().unwrap();
        let green = info["color"]["green"].as_f64().unwrap();
        let blue = info["color"]["blue"].as_f64().unwrap();
        let alpha = info["color"]["alpha"].as_f64().unwrap();
        assert!((red - 1.0).abs() < 1e-3, "red component: {red}");
        assert!(green.abs() < 1e-3, "green component: {green}");
        assert!(blue.abs() < 1e-3, "blue component: {blue}");
        assert!((alpha - 1.0).abs() < 1e-3, "alpha component: {alpha}");
    }
}

#[test]
fn color_presentation_returns_empty_array() {
    let fixture_dir = Path::new(common::FIXTURES).join("document-color");
    let mut client = LspClient::spawn(&fixture_dir);
    client.initialize();

    let uri = client.file_uri("styles.css");
    let text = std::fs::read_to_string(fixture_dir.join("styles.css")).unwrap();
    client.open_document(&uri, &text);

    let response = client.send_color_presentation(&uri);
    client.shutdown();

    let result = response.get("result").expect("expected result");
    let arr = result.as_array().expect("result should be array");
    assert!(arr.is_empty(), "expected empty array, got {arr:?}");
}
