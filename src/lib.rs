//! reasm: a configurable formatter for assembly language.
//!
//! The binary is a thin wrapper around [`driver`]. Library users can call
//! [`format_str`] directly (EXT-4).

pub mod cli;
pub mod config;
pub mod diag;
pub mod dialect;
pub mod driver;
pub mod format;
pub mod ir;
pub mod source;

pub use config::Config;
pub use format::Formatted;

/// Formats `src` with `config`, choosing the dialect from `config.dialect`
/// (or the default dialect when it is "auto"). Fails on an unknown
/// dialect, or when the safety check finds a reasm bug (ADR-0011).
pub fn format_str(src: &str, config: &Config) -> Result<Formatted, String> {
    let dialect = dialect::select(Some(&config.dialect), None)?;
    let formatted = format::format(src, None, config, dialect);
    match &formatted.check_failure {
        Some(e) => Err(format!("internal error: {e}")),
        None => Ok(formatted),
    }
}
