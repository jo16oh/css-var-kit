use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub struct Config {
    pub root_dir: PathBuf,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawConfig {
    #[serde(default)]
    root_dir: Option<String>,
}

impl RawConfig {
    fn load(project_root: &Path) -> Self {
        let candidates = ["cvk.json", "cvk.jsonc"];

        for name in candidates {
            let path = project_root.join(name);
            if let Ok(raw) = fs::read_to_string(&path) {
                let stripped = json_strip_comments::StripComments::new(raw.as_bytes());
                match serde_json::from_reader(stripped) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!("warning: failed to parse {}: {e}", path.display());
                    }
                }
            }
        }

        Self::default()
    }

    fn resolve(self, project_root: PathBuf) -> Config {
        let root_dir = match &self.root_dir {
            Some(dir) => {
                let p = Path::new(dir);
                if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    project_root.join(p)
                }
            }
            None => project_root,
        };

        Config { root_dir }
    }
}

pub fn load(cwd: &Path) -> Config {
    let project_root = find_project_root(cwd);
    RawConfig::load(&project_root).resolve(project_root)
}

fn find_project_root(cwd: &Path) -> PathBuf {
    let markers = ["cvk.json", "cvk.jsonc", "package.json", ".git"];

    for marker in markers {
        let mut dir = cwd;
        loop {
            if dir.join(marker).exists() {
                return dir.to_path_buf();
            } else if let Some(parent) = dir.parent() {
                dir = parent
            } else {
                break;
            }
        }
    }

    cwd.to_path_buf()
}

