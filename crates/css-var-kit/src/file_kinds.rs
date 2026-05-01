pub const CSS_EXTENSIONS: &[&str] = &["css", "scss"];
pub const HTML_LIKE_EXTENSIONS: &[&str] = &["html", "vue", "svelte", "astro"];
pub const CONFIG_FILENAMES: &[&str] = &["cvk.json", "cvk.jsonc"];

pub fn is_supported_source_extension(ext: &str) -> bool {
    CSS_EXTENSIONS.contains(&ext) || HTML_LIKE_EXTENSIONS.contains(&ext)
}

pub fn is_html_like_extension(ext: &str) -> bool {
    HTML_LIKE_EXTENSIONS.contains(&ext)
}

pub fn is_config_filename(name: &str) -> bool {
    CONFIG_FILENAMES.contains(&name)
}

pub fn watched_glob_patterns() -> Vec<String> {
    CSS_EXTENSIONS
        .iter()
        .chain(HTML_LIKE_EXTENSIONS.iter())
        .map(|ext| format!("**/*.{ext}"))
        .chain(CONFIG_FILENAMES.iter().map(|name| format!("**/{name}")))
        .collect()
}
