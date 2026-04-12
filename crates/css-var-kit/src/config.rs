pub mod file;
pub mod rules;

pub use file::RawConfig;

use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};
use thiserror::Error;

use crate::{
    cli::LintArgs,
    config::{
        file::{RawRules, SeverityToggle},
        rules::Rules,
    },
    rules::enforce_variable_use::config::RawEnforceVariableUse,
};

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
    #[error("rule '{rule}' requires '{dependency}' to be enabled")]
    MissingRuleDependency {
        rule: &'static str,
        dependency: &'static str,
    },
}

pub struct Config {
    pub root_dir: PathBuf,
    pub definition_files: LookupFilesMatcher,
    pub include: LookupFilesMatcher,
    pub rules: Rules,
    pub lsp_log_file: Option<PathBuf>,
}

pub const DEFAULT_INCLUDE_PATTERNS: &[&str] = &[
    "!**/node_modules/**",
    "!**/target/**",
    "!**/.git/**",
    "!**/dist/**",
    "!**/build/**",
    "!**/vendor/**",
];

#[derive(Clone)]
pub struct LookupFilesMatcher {
    patterns: Vec<LookupPattern>,
}

impl Default for LookupFilesMatcher {
    fn default() -> Self {
        Self::compile(&["**/*.css".to_string()]).unwrap()
    }
}

#[derive(Clone)]
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

    pub fn has_positive_patterns(&self) -> bool {
        self.patterns.iter().any(|p| !p.negated)
    }

    /// Returns true if a synthetic file inside `dir_rel_path` would be negated.
    /// Used as a fast pruning check to skip entire directory trees during file collection.
    pub fn is_dir_negated(&self, dir_rel_path: &Path) -> bool {
        self.is_negated(&dir_rel_path.join("_"))
    }

    /// Returns true if the last matching pattern for `path` is a negation.
    /// Used to exclude files from linting via `include: ["!**/vendor/**"]`.
    pub fn is_negated(&self, path: &Path) -> bool {
        let mut last_negated = false;
        for pattern in &self.patterns {
            if pattern.matcher.is_match(path) {
                last_negated = pattern.negated;
            }
        }
        last_negated
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
    pub fn load(cwd: &Path, args: Option<LintArgs>) -> Result<Self, ConfigError> {
        let args = args.unwrap_or_default();

        let (config_base, raw) = match &args.config {
            Some(path) => {
                let abs = cwd.join(path);
                let base = abs.parent().unwrap_or(cwd).to_path_buf();
                (base, file::RawConfig::load_from(&abs)?)
            }
            None => {
                let base = find_project_root(cwd);
                let raw = file::RawConfig::load(&base)?.unwrap_or_default();
                (base, raw)
            }
        };

        let root_dir = match &args.root_dir {
            Some(dir) => cwd.join(dir),
            None => config_base.join(&raw.root_dir),
        };

        let definition_patterns = if !args.files.is_empty() {
            args.files.as_slice()
        } else if let Some(ref df) = raw.definition_files {
            df.as_slice()
        } else {
            raw.lookup_files.as_slice()
        };

        let definition_files = LookupFilesMatcher::compile(definition_patterns)
            .map_err(|e| ConfigError::InvalidPattern { source: e })?;

        let include = compile_include(&raw.include)?;

        let raw_rules = raw.rules.override_raw_rules_by_args(args)?;
        let rules = Rules::from_raw(raw_rules)?;

        let lsp_log_file = raw.lsp.log_file.map(|p| config_base.join(p));

        Ok(Self {
            root_dir,
            definition_files,
            include,
            rules,
            lsp_log_file,
        })
    }

    /// Loads config for LSP. `cvk.json` takes precedence over `initializationOptions`;
    /// `initializationOptions` is used only when no config file is found.
    pub fn load_for_lsp(
        root_dir: &Path,
        init_options: Option<file::RawConfig>,
    ) -> Result<Self, ConfigError> {
        let project_root = find_project_root(root_dir);

        let raw = file::RawConfig::load(&project_root)?
            .or(init_options)
            .unwrap_or_default();

        let resolved_root_dir = project_root.join(&raw.root_dir);

        let definition_patterns = raw.definition_files.as_deref().unwrap_or(&raw.lookup_files);
        let definition_files = LookupFilesMatcher::compile(definition_patterns)
            .map_err(|e| ConfigError::InvalidPattern { source: e })?;

        let include = compile_include(&raw.include)?;

        let rules = Rules::from_raw(raw.rules)?;

        let lsp_log_file = raw.lsp.log_file.map(|p| project_root.join(p));

        Ok(Self {
            root_dir: resolved_root_dir,
            definition_files,
            include,
            rules,
            lsp_log_file,
        })
    }
}

impl RawRules {
    fn override_raw_rules_by_args(mut self, args: LintArgs) -> Result<RawRules, ConfigError> {
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
                "no-variable-type-mismatch" => {
                    self.no_variable_type_mismatch = Self::parse_toggle(value).map_err(&err)?;
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

    fn parse_toggle(value: &str) -> Result<SeverityToggle, String> {
        match value {
            "on" | "error" => Ok(SeverityToggle::Error),
            "warn" => Ok(SeverityToggle::Warn),
            "off" => Ok(SeverityToggle::Off),
            _ => Err("expected 'error', 'warn', 'on', or 'off'".into()),
        }
    }

    fn parse_enforce(value: &str) -> Result<RawEnforceVariableUse, String> {
        match value {
            "on" | "error" => Ok(RawEnforceVariableUse::default_on()),
            "warn" => Ok(RawEnforceVariableUse::default_on_with_severity(
                SeverityToggle::Warn,
            )),
            "off" => Ok(RawEnforceVariableUse::Off),
            v if v.starts_with('{') => serde_json::from_str(v)
                .map(RawEnforceVariableUse::Config)
                .map_err(|e| e.to_string()),
            _ => Err("expected 'error', 'warn', 'on', 'off', or a JSON object".into()),
        }
    }
}

/// Compiles an `include` matcher by prepending the default skip patterns before user-supplied
/// patterns. With last-wins semantics, user patterns can selectively override the defaults
/// (e.g. `"node_modules/my-lib/tokens.css"` overrides `"!**/node_modules/**"` for that path).
fn compile_include(user_patterns: &[String]) -> Result<LookupFilesMatcher, ConfigError> {
    let patterns: Vec<String> = DEFAULT_INCLUDE_PATTERNS
        .iter()
        .map(|s| s.to_string())
        .chain(user_patterns.iter().cloned())
        .collect();
    LookupFilesMatcher::compile(&patterns).map_err(|e| ConfigError::InvalidPattern { source: e })
}

pub fn find_project_root(cwd: &Path) -> PathBuf {
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
