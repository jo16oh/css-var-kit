use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::ConfigError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawConfig {
    #[serde(default = "default_root_dir")]
    pub(super) root_dir: String,
    #[serde(default = "default_lookup_files")]
    pub(super) lookup_files: Vec<String>,
}

impl Default for RawConfig {
    fn default() -> Self {
        Self {
            root_dir: default_root_dir(),
            lookup_files: default_lookup_files(),
        }
    }
}

fn default_root_dir() -> String {
    ".".to_string()
}

fn default_lookup_files() -> Vec<String> {
    vec!["**/*.css".to_string()]
}

impl RawConfig {
    pub(super) fn load(project_root: &Path) -> Result<Self, ConfigError> {
        let candidates = ["cvk.json", "cvk.jsonc"];

        for name in candidates {
            let path = project_root.join(name);
            if let Ok(raw) = fs::read_to_string(&path) {
                let stripped = json_strip_comments::StripComments::new(raw.as_bytes());
                return serde_json::from_reader(stripped)
                    .map_err(|e| ConfigError::Parse { path, source: e });
            }
        }

        Ok(Self::default())
    }
}
