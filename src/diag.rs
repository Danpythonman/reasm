//! Diagnostics: warnings and notes produced while formatting a file.
//! Rendered as `path:line:col: severity[CODE]: message` (NFR-6).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Note,
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Note => "note",
            Self::Warning => "warning",
            Self::Error => "error",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable code such as `W001`, usable with `--allow` (ARCHITECTURE §7).
    pub code: &'static str,
    /// 1-based line and column; 0 means "not tied to a position".
    pub line: u32,
    pub col: u32,
    pub message: String,
}

impl Diagnostic {
    pub fn render(&self, path: &str) -> String {
        let at = match (self.line, self.col) {
            (0, _) => format!("{path}:"),
            (line, 0) => format!("{path}:{line}:"),
            (line, col) => format!("{path}:{line}:{col}:"),
        };
        format!("{at} {}[{}]: {}", self.severity, self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_formats() {
        let d = Diagnostic {
            severity: Severity::Warning,
            code: "W001",
            line: 3,
            col: 7,
            message: "could not parse line".into(),
        };
        assert_eq!(
            d.render("a.asm"),
            "a.asm:3:7: warning[W001]: could not parse line"
        );
        let d = Diagnostic { line: 0, ..d };
        assert_eq!(
            d.render("a.asm"),
            "a.asm: warning[W001]: could not parse line"
        );
    }
}
