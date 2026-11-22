//! Dialect abstraction (ADR-0004). Everything dialect-specific sits behind
//! [`Dialect`]; the formatter core never names a concrete dialect.

use std::path::Path;

use crate::config::Config;
use crate::diag::Diagnostic;
use crate::ir::Document;
use crate::source::Source;

pub mod nasm;

pub trait Dialect: Sync {
    /// Name used by `--dialect` and the `dialect` config key.
    fn name(&self) -> &'static str;

    /// File extensions (without the dot) that select this dialect.
    fn extensions(&self) -> &'static [&'static str];

    /// Whether the trailing whitespace of `line` changes its meaning, so it
    /// must not be trimmed. For NASM, `\` followed by spaces is *not* a line
    /// continuation, while a bare trailing `\` is (NASM.md §1).
    fn trailing_whitespace_significant(&self, line: &str) -> bool;

    /// Parses the source into the IR. Must return exactly one IR line per
    /// source line, and never fail: what can't be parsed becomes a
    /// verbatim line plus a warning (ADR-0006).
    fn parse(&self, source: &Source, config: &Config, diags: &mut Vec<Diagnostic>) -> Document;
}

/// All supported dialects. The first one is the fallback.
static REGISTRY: &[&dyn Dialect] = &[&nasm::Nasm];

pub fn by_name(name: &str) -> Option<&'static dyn Dialect> {
    REGISTRY
        .iter()
        .copied()
        .find(|d| d.name().eq_ignore_ascii_case(name))
}

pub fn names() -> Vec<&'static str> {
    REGISTRY.iter().map(|d| d.name()).collect()
}

pub fn default_dialect() -> &'static dyn Dialect {
    REGISTRY[0]
}

/// Picks the dialect for a file: an explicit name wins (`--dialect`, then
/// the config `dialect` key unless it is "auto"), then the file extension,
/// then the fallback.
pub fn select(explicit: Option<&str>, path: Option<&Path>) -> Result<&'static dyn Dialect, String> {
    if let Some(name) = explicit.filter(|n| !n.eq_ignore_ascii_case("auto")) {
        return by_name(name).ok_or_else(|| {
            format!(
                "unknown dialect `{name}` (available: {})",
                names().join(", ")
            )
        });
    }
    let ext = path.and_then(Path::extension).and_then(|e| e.to_str());
    if let Some(ext) = ext {
        let found = REGISTRY
            .iter()
            .copied()
            .find(|d| d.extensions().iter().any(|e| e.eq_ignore_ascii_case(ext)));
        if let Some(d) = found {
            return Ok(d);
        }
    }
    Ok(default_dialect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_order() {
        assert_eq!(select(None, None).unwrap().name(), "nasm");
        assert_eq!(
            select(Some("auto"), Some(Path::new("x.ASM")))
                .unwrap()
                .name(),
            "nasm"
        );
        assert_eq!(select(Some("NASM"), None).unwrap().name(), "nasm");
        assert!(select(Some("gas"), None).is_err());
    }
}
