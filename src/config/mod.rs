//! Configuration: option types, defaults, TOML loading, `--set` overrides,
//! discovery and printing. See docs/CONFIGURATION.md and ADR-0005.
//!
//! The field order of every struct here is the order `--print-config` prints,
//! and must match the "Full default file" block in CONFIGURATION.md (a test
//! enforces this).

mod load;
mod options;

pub use load::{CONFIG_FILE_NAMES, ConfigError, Override, discover, load};
pub use options::*;
