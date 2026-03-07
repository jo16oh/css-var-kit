mod file;
mod rules;

use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};
use thiserror::Error;

pub use rules::{EnforceVariableUse, Rules};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("invalid lookupFiles pattern: {source}")]
    InvalidPattern { source: globset::Error },
}

pub struct Config {
    pub root_dir: PathBuf,
    pub lookup_files: LookupFilesMatcher,
    pub rules: Rules,
}

pub struct LookupFilesMatcher {
    patterns: Vec<LookupPattern>,
}

struct LookupPattern {
    negated: bool,
    matcher: GlobMatcher,
}

impl LookupFilesMatcher {
    fn compile(raw_patterns: &[String]) -> Result<Self, globset::Error> {
        raw_patterns
            .iter()
            .map(|raw| {
                let (negated, pat) = match raw.strip_prefix('!') {
                    Some(rest) => (true, rest),
                    None => (false, raw.as_str()),
                };
                Ok(LookupPattern {
                    negated,
                    matcher: Glob::new(pat)?.compile_matcher(),
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|patterns| Self { patterns })
    }

    pub fn matches(&self, path: &Path) -> bool {
        let mut matched = false;
        for pattern in &self.patterns {
            if pattern.matcher.is_match(path) {
                matched = !pattern.negated;
            }
        }
        matched
    }
}

impl Config {
    pub fn load(cwd: &Path) -> Result<Self, ConfigError> {
        let project_root = find_project_root(cwd);
        let raw = file::RawConfig::load(&project_root)?;

        let root_dir = project_root.join(&raw.root_dir);
        let lookup_files = LookupFilesMatcher::compile(&raw.lookup_files)
            .map_err(|e| ConfigError::InvalidPattern { source: e })?;
        let rules = Rules::from_raw(raw.rules);

        Ok(Self {
            root_dir,
            lookup_files,
            rules,
        })
    }
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
