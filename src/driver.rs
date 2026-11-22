//! The driver: finds files, resolves each file's config, formats, and writes,
//! checks or prints the result. See docs/CLI.md for the user-visible contract.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::config::{self, Config, ConfigError, Override};
use crate::diag::Severity;
use crate::dialect;
use crate::format;

/// Exit codes (CLI.md). When several apply, the highest wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Exit {
    Ok = 0,
    WouldChange = 1,
    UserError = 2,
    Internal = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Rewrite changed files in place (ADR-0013).
    Write,
    /// Write nothing; report files that would change.
    Check,
    /// Print formatted output to stdout.
    Stdout,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigChoice {
    Discover,
    File(PathBuf),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub mode: Mode,
    pub config: ConfigChoice,
    pub overrides: Vec<Override>,
    pub dialect: Option<String>,
    pub allow: Vec<String>,
    pub deny_warnings: bool,
    pub verbosity: Verbosity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Stdin { filepath: Option<PathBuf> },
    Paths(Vec<PathBuf>),
}

/// Runs a formatting job. Messages go to `out` (formatted text, `--check`
/// file lists) and `err` (diagnostics, errors).
pub fn run(input: &Input, opts: &Options, out: &mut dyn Write, err: &mut dyn Write) -> Exit {
    let mut job = Job {
        opts,
        configs: ConfigCache::default(),
        exit: Exit::Ok,
        err,
    };
    match input {
        Input::Stdin { filepath } => job.run_stdin(filepath.as_deref(), out),
        Input::Paths(paths) => job.run_paths(paths, out),
    }
    job.exit
}

/// Resolves the effective config for `dir` (or the given file) and returns it
/// as TOML, for `--print-config current`.
pub fn effective_config(opts: &Options, dir: &Path) -> Result<String, ConfigError> {
    let mut cache = ConfigCache::default();
    Ok(cache.for_dir(opts, dir)?.to_toml())
}

#[derive(Default)]
struct ConfigCache {
    /// Directory → config file that applies to it.
    discovered: HashMap<PathBuf, Option<PathBuf>>,
    /// Config file (or none) → loaded config.
    loaded: HashMap<Option<PathBuf>, Rc<Config>>,
}

impl ConfigCache {
    fn for_dir(&mut self, opts: &Options, dir: &Path) -> Result<Rc<Config>, ConfigError> {
        let file = match &opts.config {
            ConfigChoice::None => None,
            ConfigChoice::File(f) => Some(f.clone()),
            ConfigChoice::Discover => match self.discovered.get(dir) {
                Some(found) => found.clone(),
                None => {
                    let found = config::discover(dir)?;
                    self.discovered.insert(dir.to_path_buf(), found.clone());
                    found
                }
            },
        };
        if let Some(c) = self.loaded.get(&file) {
            return Ok(Rc::clone(c));
        }
        let c = Rc::new(config::load(file.as_deref(), &opts.overrides)?);
        self.loaded.insert(file, Rc::clone(&c));
        Ok(c)
    }
}

struct Job<'a> {
    opts: &'a Options,
    configs: ConfigCache,
    exit: Exit,
    err: &'a mut dyn Write,
}

/// What to do with one piece of formatted text.
enum Target<'p> {
    File(&'p Path),
    Stdin,
}

impl Job<'_> {
    fn raise(&mut self, exit: Exit) {
        self.exit = self.exit.max(exit);
    }

    fn error(&mut self, message: impl std::fmt::Display) {
        let _ = writeln!(self.err, "error: {message}");
        self.raise(Exit::UserError);
    }

    fn run_stdin(&mut self, filepath: Option<&Path>, out: &mut dyn Write) {
        let mut bytes = Vec::new();
        if let Err(e) = io::stdin().read_to_end(&mut bytes) {
            return self.error(format_args!("cannot read stdin: {e}"));
        }
        let dir = match filepath.and_then(Path::parent) {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let display = filepath.map_or_else(|| "<stdin>".to_string(), |p| p.display().to_string());
        self.format_one(&bytes, &display, &dir, filepath, Target::Stdin, out);
    }

    fn run_paths(&mut self, paths: &[PathBuf], out: &mut dyn Write) {
        let mut files = Vec::new();
        for path in paths {
            if path.is_dir() {
                self.walk(path, &mut files);
            } else if path.is_file() {
                files.push(path.clone());
            } else {
                self.error(format_args!(
                    "{}: no such file or directory",
                    path.display()
                ));
            }
        }
        let headers = self.opts.mode == Mode::Stdout && files.len() > 1;
        for file in &files {
            let display = file.display().to_string();
            if headers {
                let _ = writeln!(out, "==> {display} <==");
            }
            let bytes = match std::fs::read(file) {
                Ok(b) => b,
                Err(e) => {
                    self.error(format_args!("{display}: {e}"));
                    continue;
                }
            };
            let dir = match file.parent() {
                Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                _ => PathBuf::from("."),
            };
            self.format_one(&bytes, &display, &dir, Some(file), Target::File(file), out);
        }
    }

    /// Collects files under `root` whose extension is in the root's config
    /// `extensions` (case-insensitive), skipping hidden directories and
    /// `target/`. Output is sorted for deterministic order.
    fn walk(&mut self, root: &Path, files: &mut Vec<PathBuf>) {
        let config = match self.configs.for_dir(self.opts, root) {
            Ok(c) => c,
            Err(e) => return self.error(e),
        };
        let mut stack = vec![root.to_path_buf()];
        let mut found = Vec::new();
        while let Some(dir) = stack.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(e) => {
                    self.error(format_args!("{}: {e}", dir.display()));
                    continue;
                }
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name();
                let name = name.to_string_lossy();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    if !name.starts_with('.') && name != "target" {
                        stack.push(path);
                    }
                } else if kind.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if config
                        .extensions
                        .iter()
                        .any(|e| e.eq_ignore_ascii_case(ext))
                    {
                        found.push(path);
                    }
                }
            }
        }
        found.sort();
        files.extend(found);
    }

    fn format_one(
        &mut self,
        bytes: &[u8],
        display: &str,
        dir: &Path,
        path: Option<&Path>,
        target: Target<'_>,
        out: &mut dyn Write,
    ) {
        let text = match std::str::from_utf8(bytes) {
            Ok(t) => t,
            Err(e) => {
                return self.error(format_args!(
                    "{display}: not valid UTF-8 (at byte {}); reasm only reads UTF-8",
                    e.valid_up_to()
                ));
            }
        };
        let config = match self.configs.for_dir(self.opts, dir) {
            Ok(c) => c,
            Err(e) => return self.error(e),
        };
        let explicit = self.opts.dialect.as_deref().unwrap_or(&config.dialect);
        let dialect = match dialect::select(Some(explicit), path) {
            Ok(d) => d,
            Err(e) => return self.error(e),
        };

        let formatted = format::format(text, &config, dialect);

        let deny = self.opts.deny_warnings || config.diagnostics.deny_warnings;
        for d in &formatted.diagnostics {
            let allowed = self
                .opts
                .allow
                .iter()
                .chain(&config.diagnostics.allow)
                .any(|a| a == d.code);
            let shown = match d.severity {
                Severity::Error => true,
                Severity::Warning => !allowed && self.opts.verbosity >= Verbosity::Normal,
                Severity::Note => !allowed && self.opts.verbosity >= Verbosity::Verbose,
            };
            if shown {
                let _ = writeln!(self.err, "{}", d.render(display));
            }
            let fails = d.severity == Severity::Error
                || (d.severity == Severity::Warning && deny && !allowed);
            if fails {
                self.raise(Exit::UserError);
            }
        }

        let changed = formatted.text != text;
        match (self.opts.mode, target) {
            (Mode::Check, _) => {
                if changed {
                    let _ = writeln!(out, "{display}");
                    self.raise(Exit::WouldChange);
                }
            }
            (Mode::Stdout, _) | (Mode::Write, Target::Stdin) => {
                if let Err(e) = out.write_all(formatted.text.as_bytes()) {
                    self.error(format_args!("cannot write output: {e}"));
                }
            }
            (Mode::Write, Target::File(file)) => {
                if changed && let Err(e) = write_atomic(file, formatted.text.as_bytes()) {
                    self.error(format_args!("{display}: cannot write: {e}"));
                }
            }
        }
    }
}

/// Writes via a temporary file in the same directory and a rename, keeping
/// the original file's permissions (ARCHITECTURE §6).
fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".reasm.tmp");
    let tmp = path.with_file_name(tmp_name);
    let result = (|| {
        let perms = std::fs::metadata(path)?.permissions();
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()?;
        drop(f);
        std::fs::set_permissions(&tmp, perms)?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
