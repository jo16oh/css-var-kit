mod file;
mod rules;

use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};
use thiserror::Error;

use crate::{
    cli::LintArgs,
    config::file::{RawEnforceVariableUse, RawRules, Toggle},
};
pub use rules::{EnforceVariableUse, Rules};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("cannot read config file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid lookupFiles pattern: {source}")]
    InvalidPattern { source: globset::Error },
    #[error("invalid --rule value '{raw}': {reason}")]
    InvalidRuleOption { raw: String, reason: String },
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
    pub fn load(cwd: &Path, args: &LintArgs) -> Result<Self, ConfigError> {
        let (config_base, raw) = match &args.config {
            Some(path) => {
                let abs = cwd.join(path);
                let base = abs.parent().unwrap_or(cwd).to_path_buf();
                (base, file::RawConfig::load_from(&abs)?)
            }
            None => {
                let base = find_project_root(cwd);
                let raw = file::RawConfig::load(&base)?;
                (base, raw)
            }
        };

        let root_dir = match &args.root_dir {
            Some(dir) => cwd.join(dir),
            None => config_base.join(&raw.root_dir),
        };

        let lookup_patterns = if args.files.is_empty() {
            &raw.lookup_files
        } else {
            &args.files
        };

        let lookup_files = LookupFilesMatcher::compile(lookup_patterns)
            .map_err(|e| ConfigError::InvalidPattern { source: e })?;

        let raw_rules = raw.rules.override_raw_rules_by_args(args)?;
        let rules = Rules::from_raw(raw_rules);

        Ok(Self {
            root_dir,
            lookup_files,
            rules,
        })
    }
}

impl RawRules {
    fn override_raw_rules_by_args(mut self, args: &LintArgs) -> Result<RawRules, ConfigError> {
        for entry in &args.rule {
            let err = |reason: String| ConfigError::InvalidRuleOption {
                raw: entry.clone(),
                reason,
            };
            let (name, value) = entry
                .split_once('=')
                .ok_or_else(|| err("expected format NAME=VALUE".into()))?;

            match name {
                "no-undefined-variable-use" => {
                    self.no_undefined_variable_use = Self::parse_toggle(value).map_err(&err)?;
                }
                "no-compound-value-in-definition" => {
                    self.no_compound_value_in_definition =
                        Self::parse_toggle(value).map_err(&err)?;
                }
                "no-variable-type-mismatch" => {
                    self.no_variable_type_mismatch =
                        Self::parse_toggle(value).map_err(&err)?;
                }
                "no-inconsistent-variable-definition" => {
                    self.no_inconsistent_variable_definition =
                        Self::parse_toggle(value).map_err(&err)?;
                }
                "enforce-variable-use" => {
                    self.enforce_variable_use = Self::parse_enforce(value).map_err(&err)?;
                }
                _ => return Err(err(format!("unknown rule '{name}'"))),
            }
        }

        Ok(self)
    }

    fn parse_toggle(value: &str) -> Result<Toggle, String> {
        match value {
            "on" => Ok(Toggle::On),
            "off" => Ok(Toggle::Off),
            _ => Err("expected 'on' or 'off'".into()),
        }
    }

    fn parse_enforce(value: &str) -> Result<RawEnforceVariableUse, String> {
        match value {
            "on" => Ok(RawEnforceVariableUse::default_on()),
            "off" => Ok(RawEnforceVariableUse::Off),
            v if v.starts_with('{') => serde_json::from_str(v)
                .map(RawEnforceVariableUse::Config)
                .map_err(|e| e.to_string()),
            _ => Err("expected 'on', 'off', or a JSON object".into()),
        }
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
