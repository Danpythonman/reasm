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
pub mod source;

pub use config::Config;
pub use format::Formatted;

/// Formats `src` with `config`, choosing the dialect from `config.dialect`
/// (or the default dialect when it is "auto").
pub fn format_str(src: &str, config: &Config) -> Result<Formatted, String> {
    let dialect = dialect::select(Some(&config.dialect), None)?;
    Ok(format::format(src, config, dialect))
}
