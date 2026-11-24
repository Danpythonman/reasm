//! Command line parsing (clap, ADR-0012). docs/CLI.md is the contract.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::assemble::Assembler;
use crate::config::{Config, Override};
use crate::driver::{self, ConfigChoice, Exit, Input, Mode, Options, Verbosity};

#[derive(Debug, Parser)]
#[command(
    name = "reasm",
    version,
    about = "A configurable formatter for assembly language (NASM)",
    long_about = "A configurable formatter for assembly language (NASM).\n\n\
        Files are formatted in place. Directories are searched recursively for the \
        configured extensions. With no paths, or with `-`, reads stdin and writes stdout.",
    after_help = "Configuration is read from the nearest reasm.toml or .reasm.toml. \
        Run `reasm --print-config default` to see every option."
)]
pub struct Args {
    /// Files or directories to format. Omit, or use `-`, for stdin.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Write nothing; list files that would change and exit 1 if any would.
    #[arg(long, conflicts_with_all = ["stdout", "diff"])]
    pub check: bool,

    /// Write nothing; print a unified diff of each file that would change
    /// and exit 1 if any would.
    #[arg(long, conflicts_with = "stdout")]
    pub diff: bool,

    /// Also assemble the original and formatted text with nasm and compare
    /// the output; a file that differs is left unchanged. FORMAT defaults to
    /// elf64, then bin.
    #[arg(long, value_name = "FORMAT", num_args = 0..=1, require_equals = true)]
    pub verify: Option<Option<String>>,

    /// Print formatted output instead of writing files.
    #[arg(long)]
    pub stdout: bool,

    /// Treat stdin as this file, for config discovery, dialect and messages.
    #[arg(long, value_name = "PATH")]
    pub stdin_filepath: Option<PathBuf>,

    /// Use this config file instead of searching for one.
    #[arg(long, value_name = "FILE", conflicts_with = "no_config")]
    pub config: Option<PathBuf>,

    /// Ignore config files; use defaults and --set only.
    #[arg(long)]
    pub no_config: bool,

    /// Override a config key, e.g. --set comments.column=48. Repeatable.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub set: Vec<String>,

    /// Force a dialect instead of choosing by file extension.
    #[arg(long, value_name = "NAME")]
    pub dialect: Option<String>,

    /// Print the default or effective configuration as TOML and exit.
    #[arg(long, value_name = "WHICH", num_args = 0..=1, default_missing_value = "current")]
    pub print_config: Option<PrintConfig>,

    /// Silence a warning code (e.g. W001). Repeatable.
    #[arg(long, value_name = "CODE")]
    pub allow: Vec<String>,

    /// Treat warnings as errors.
    #[arg(long)]
    pub deny_warnings: bool,

    /// Only print errors.
    #[arg(short, long, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Also print notes.
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PrintConfig {
    Default,
    Current,
}

/// Parses `std::env::args` and runs. Returns the process exit code.
pub fn main() -> u8 {
    let args = Args::parse();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    run(args, &mut stdout.lock(), &mut stderr.lock()) as u8
}

pub fn run(args: Args, out: &mut dyn Write, err: &mut dyn Write) -> Exit {
    let mut fail = |message: String| {
        let _ = writeln!(err, "error: {message}");
        Exit::UserError
    };

    let mut overrides = Vec::with_capacity(args.set.len());
    for s in &args.set {
        match Override::parse(s) {
            Ok(o) => overrides.push(o),
            Err(e) => return fail(e.to_string()),
        }
    }
    let config = match (&args.config, args.no_config) {
        (Some(f), _) => ConfigChoice::File(f.clone()),
        (None, true) => ConfigChoice::None,
        (None, false) => ConfigChoice::Discover,
    };
    let mode = if args.check {
        Mode::Check
    } else if args.diff {
        Mode::Diff
    } else if args.stdout {
        Mode::Stdout
    } else {
        Mode::Write
    };
    let verbosity = if args.quiet {
        Verbosity::Quiet
    } else if args.verbose {
        Verbosity::Verbose
    } else {
        Verbosity::Normal
    };
    let opts = Options {
        mode,
        config,
        overrides,
        dialect: args.dialect.clone(),
        allow: args.allow.clone(),
        deny_warnings: args.deny_warnings,
        verbosity,
        verify: None,
    };

    if let Some(which) = args.print_config {
        let text = match which {
            PrintConfig::Default => Ok(Config::default().to_toml()),
            PrintConfig::Current => {
                let dir = match args.stdin_filepath.as_deref().and_then(|p| p.parent()) {
                    Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                    _ => PathBuf::from("."),
                };
                driver::effective_config(&opts, &dir)
            }
        };
        return match text {
            Ok(t) => {
                let _ = out.write_all(t.as_bytes());
                Exit::Ok
            }
            Err(e) => fail(e.to_string()),
        };
    }

    let mut opts = opts;
    if let Some(format) = &args.verify {
        let asm = Assembler::new(format.as_deref());
        if let Err(e) = asm.probe() {
            return fail(format!(
                "--verify needs nasm on PATH: cannot run `nasm`: {e}"
            ));
        }
        opts.verify = Some(asm);
    }

    let stdin_requested = args.paths.is_empty() || args.paths.iter().any(|p| p.as_os_str() == "-");
    let input = if stdin_requested {
        if args.paths.len() > 1 {
            return fail("`-` (stdin) cannot be combined with other paths".into());
        }
        if std::io::stdin().is_terminal() {
            return fail(
                "no input: pass files or directories, or pipe source on stdin \
                         (see --help)"
                    .into(),
            );
        }
        Input::Stdin {
            filepath: args.stdin_filepath.clone(),
        }
    } else {
        if args.stdin_filepath.is_some() {
            return fail("--stdin-filepath only applies when reading stdin".into());
        }
        Input::Paths(args.paths.clone())
    };

    driver::run(&input, &opts, out, err)
}
