mod file;
mod rules;

use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};
use thiserror::Error;

use crate::cli::LintArgs;
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
        let project_root = find_project_root(cwd);

        let raw = match &args.config {
            Some(path) => file::RawConfig::load_from(Path::new(path))?,
            None => file::RawConfig::load(&project_root)?,
        };

        let root_dir = match &args.root_dir {
            Some(dir) => cwd.join(dir),
            None => project_root.join(&raw.root_dir),
        };

        let lookup_patterns = if args.files.is_empty() {
            &raw.lookup_files
        } else {
            &args.files
        };

        let lookup_files = LookupFilesMatcher::compile(lookup_patterns)
            .map_err(|e| ConfigError::InvalidPattern { source: e })?;

        let mut rules = Rules::from_raw(raw.rules);
        apply_rule_overrides(&mut rules, &args.rule)?;

        Ok(Self {
            root_dir,
            lookup_files,
            rules,
        })
    }
}

fn apply_rule_overrides(rules: &mut Rules, overrides: &[String]) -> Result<(), ConfigError> {
    for entry in overrides {
        let (name, value) =
            entry
                .split_once('=')
                .ok_or_else(|| ConfigError::InvalidRuleOption {
                    raw: entry.clone(),
                    reason: "expected format NAME=VALUE".to_string(),
                })?;

        match name {
            "no-undefined-variable-use" => {
                rules.no_undefined_variable_use = parse_toggle(value, entry)?;
            }
            "no-compound-value-in-definition" => {
                rules.no_compound_value_in_definition = parse_toggle(value, entry)?;
            }
            "no-type-mismatch" => {
                rules.no_type_mismatch = parse_toggle(value, entry)?;
            }
            "enforce-variable-use" => {
                if value == "off" {
                    rules.enforce_variable_use = None;
                } else if value == "on" {
                    // on keeps existing config; if none exists, rule stays effectively disabled
                    // (no types configured)
                } else if value.starts_with('{') {
                    let raw: file::RawEnforceVariableUseConfig = serde_json::from_str(value)
                        .map_err(|e| ConfigError::InvalidRuleOption {
                            raw: entry.clone(),
                            reason: e.to_string(),
                        })?;
                    let config = EnforceVariableUse::from_raw(raw);
                    if config.types.is_empty() {
                        rules.enforce_variable_use = None;
                    } else {
                        rules.enforce_variable_use = Some(config);
                    }
                } else {
                    return Err(ConfigError::InvalidRuleOption {
                        raw: entry.clone(),
                        reason: "expected 'on', 'off', or a JSON object".to_string(),
                    });
                }
            }
            _ => {
                return Err(ConfigError::InvalidRuleOption {
                    raw: entry.clone(),
                    reason: format!("unknown rule '{name}'"),
                });
            }
        }
    }
    Ok(())
}

fn parse_toggle(value: &str, raw: &str) -> Result<bool, ConfigError> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(ConfigError::InvalidRuleOption {
            raw: raw.to_string(),
            reason: "expected 'on' or 'off'".to_string(),
        }),
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
