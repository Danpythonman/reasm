//! Loading a config file, applying `--set` overrides, and discovery.

use std::fmt;
use std::path::{Path, PathBuf};

use super::Config;

/// File names searched for in each directory, in this order (ADR-0005).
pub const CONFIG_FILE_NAMES: [&str; 2] = ["reasm.toml", ".reasm.toml"];

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
    /// Invalid TOML, an unknown key, a wrong type, or a failed range check in a file.
    File { path: PathBuf, message: String },
    /// A malformed `--set` argument, or an invalid value it introduced.
    Override { message: String },
    /// Both `reasm.toml` and `.reasm.toml` exist in one directory.
    Ambiguous { dir: PathBuf },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => {
                write!(f, "{}: cannot read config: {error}", path.display())
            }
            Self::File { path, message } => write!(f, "{}: {}", path.display(), message.trim_end()),
            Self::Override { message } => write!(f, "--set: {}", message.trim_end()),
            Self::Ambiguous { dir } => write!(
                f,
                "{}: both reasm.toml and .reasm.toml exist; keep only one",
                dir.display()
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

/// One `--set key.path=value` override.
#[derive(Debug, Clone, PartialEq)]
pub struct Override {
    pub path: Vec<String>,
    pub value: toml::Value,
}

impl Override {
    /// Parses `key.path=value`. The value is read as a TOML value when it is
    /// one (`4`, `true`, `["asm"]`, `"x"`); anything else is taken as a bare
    /// string, so `--set case.mnemonic=upper` works without quotes.
    pub fn parse(arg: &str) -> Result<Self, ConfigError> {
        let err = |message: String| ConfigError::Override { message };
        let (key, raw) = arg
            .split_once('=')
            .ok_or_else(|| err(format!("`{arg}`: expected KEY=VALUE")))?;
        let key = key.trim();
        let path: Vec<String> = key.split('.').map(|s| s.trim().to_string()).collect();
        if path.iter().any(String::is_empty) {
            return Err(err(format!("`{arg}`: invalid key `{key}`")));
        }
        let raw = raw.trim();
        let value = toml::from_str::<toml::Table>(&format!("v = {raw}"))
            .ok()
            .and_then(|mut t| t.remove("v"))
            .unwrap_or_else(|| toml::Value::String(raw.to_string()));
        Ok(Self { path, value })
    }

    /// Checks the key path against the shape of the default config, so the
    /// error says *which* segment is wrong. Leaf names are left to serde's
    /// `deny_unknown_fields`, which lists the valid keys.
    fn check_path(&self, defaults: &toml::Table) -> Result<(), ConfigError> {
        let mut table = defaults;
        for (i, segment) in self.path[..self.path.len() - 1].iter().enumerate() {
            let here = self.path[..=i].join(".");
            table = match table.get(segment) {
                Some(toml::Value::Table(t)) => t,
                Some(_) => {
                    return Err(ConfigError::Override {
                        message: format!("`{here}` is not a table"),
                    });
                }
                None => {
                    return Err(ConfigError::Override {
                        message: format!("unknown key `{here}`"),
                    });
                }
            };
        }
        Ok(())
    }

    fn apply(&self, root: &mut toml::Table) -> Result<(), ConfigError> {
        let (last, parents) = self.path.split_last().expect("path is non-empty");
        let mut table = root;
        for (i, segment) in parents.iter().enumerate() {
            let entry = table
                .entry(segment.clone())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
            table = entry.as_table_mut().ok_or_else(|| ConfigError::Override {
                message: format!("`{}` is not a table", self.path[..=i].join(".")),
            })?;
        }
        table.insert(last.clone(), self.value.clone());
        Ok(())
    }
}

/// Builds the effective config: defaults, then `file` (if any), then `overrides`.
pub fn load(file: Option<&Path>, overrides: &[Override]) -> Result<Config, ConfigError> {
    let mut table = toml::Table::new();
    if let Some(path) = file {
        let text = std::fs::read_to_string(path).map_err(|error| ConfigError::Io {
            path: path.to_path_buf(),
            error,
        })?;
        let file_err = |message: String| ConfigError::File {
            path: path.to_path_buf(),
            message,
        };
        // Deserialize the text directly first: this gives errors with line
        // and column, which deserializing from a `Table` would lose.
        let config: Config = toml::from_str(&text).map_err(|e| file_err(e.to_string()))?;
        config.validate().map_err(file_err)?;
        if overrides.is_empty() {
            return Ok(config);
        }
        table = toml::from_str(&text).map_err(|e| file_err(e.to_string()))?;
    }

    let defaults = toml::Table::try_from(Config::default()).expect("Config serializes to a table");
    for o in overrides {
        o.check_path(&defaults)?;
        o.apply(&mut table)?;
    }
    let config: Config = toml::Value::Table(table)
        .try_into()
        .map_err(|e: toml::de::Error| ConfigError::Override {
            message: e.to_string(),
        })?;
    config
        .validate()
        .map_err(|message| ConfigError::Override { message })?;
    Ok(config)
}

/// Finds the config file that applies to files in `dir`: the nearest
/// `reasm.toml` or `.reasm.toml` in `dir` or its ancestors.
pub fn discover(dir: &Path) -> Result<Option<PathBuf>, ConfigError> {
    let start = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    for ancestor in start.ancestors() {
        let mut found = CONFIG_FILE_NAMES
            .iter()
            .map(|n| ancestor.join(n))
            .filter(|p| p.is_file());
        if let Some(first) = found.next() {
            if found.next().is_some() {
                return Err(ConfigError::Ambiguous {
                    dir: ancestor.to_path_buf(),
                });
            }
            return Ok(Some(first));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CaseStyle, SplitInline};

    fn ov(s: &str) -> Override {
        Override::parse(s).unwrap()
    }

    #[test]
    fn override_value_types() {
        assert_eq!(ov("indent.instruction=4").value, toml::Value::Integer(4));
        assert_eq!(
            ov("labels.add_colon=true").value,
            toml::Value::Boolean(true)
        );
        assert_eq!(
            ov("case.mnemonic=upper").value,
            toml::Value::String("upper".into())
        );
        assert_eq!(
            ov("case.mnemonic=\"upper\"").value,
            toml::Value::String("upper".into())
        );
        assert!(ov("extensions=[\"asm\"]").value.is_array());
        assert_eq!(ov(" a.b = x ").path, ["a", "b"]);
    }

    #[test]
    fn override_parse_errors() {
        assert!(Override::parse("indent.instruction").is_err());
        assert!(Override::parse("indent..x=1").is_err());
        assert!(Override::parse("=1").is_err());
    }

    #[test]
    fn overrides_apply_over_defaults() {
        let c = load(
            None,
            &[ov("case.mnemonic=upper"), ov("labels.split_inline=code")],
        )
        .unwrap();
        assert_eq!(c.case.mnemonic, CaseStyle::Upper);
        assert_eq!(c.labels.split_inline, SplitInline::Code);
        assert_eq!(c.indent, Config::default().indent);
    }

    #[test]
    fn override_errors() {
        let unknown = load(None, &[ov("labels.nope=1")]).unwrap_err().to_string();
        assert!(unknown.contains("nope"), "{unknown}");
        let not_table = load(None, &[ov("dialect.x=1")]).unwrap_err().to_string();
        assert!(
            not_table.contains("`dialect` is not a table"),
            "{not_table}"
        );
        let bad_table = load(None, &[ov("lables.indent=1")])
            .unwrap_err()
            .to_string();
        assert!(bad_table.contains("unknown key `lables`"), "{bad_table}");
        let range = load(None, &[ov("indent.tab_width=0")])
            .unwrap_err()
            .to_string();
        assert!(range.contains("indent.tab_width"), "{range}");
        let value = load(None, &[ov("case.mnemonic=sideways")])
            .unwrap_err()
            .to_string();
        assert!(value.contains("sideways"), "{value}");
    }

    #[test]
    fn validation_rules() {
        let mut c = Config::default();
        assert!(c.validate().is_ok());
        c.dialect = "gas".into();
        assert!(c.validate().unwrap_err().contains("unknown dialect"));
        c = Config::default();
        c.extensions = vec![".asm".into()];
        assert!(c.validate().unwrap_err().contains("without the dot"));
        c = Config::default();
        c.comments.min_gap = 0;
        assert!(c.validate().unwrap_err().contains("comments.min_gap"));
    }

    #[test]
    fn default_round_trips_through_toml() {
        let c = Config::default();
        let back: Config = toml::from_str(&c.to_toml()).unwrap();
        assert_eq!(back, c);
    }
}
