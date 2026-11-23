//! The formatter core (ARCHITECTURE §5): parse → normalize → layout →
//! emit → safety check.

pub mod emit;
pub mod layout;
pub mod normalize;
pub mod verify;

use crate::config::{Config, LineEndingStyle};
use crate::diag::Diagnostic;
use crate::dialect::Dialect;
use crate::source::{LineEnding, Source};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formatted {
    /// The formatted text, or the input unchanged when `check_failure`
    /// is set.
    pub text: String,
    pub diagnostics: Vec<Diagnostic>,
    /// Set when the safety check (ADR-0011) found a difference. This is
    /// a reasm bug: the caller must not write the file (exit 3).
    pub check_failure: Option<String>,
}

/// Formats `src`. `path` is where it was read from, if anywhere; the
/// dialect may read included files relative to it (ADR-0022).
pub fn format(src: &str, path: Option<&Path>, config: &Config, dialect: &dyn Dialect) -> Formatted {
    let source = Source::new(src);
    let newline = match config.line_ending {
        LineEndingStyle::Preserve => source.line_ending,
        LineEndingStyle::Lf => LineEnding::Lf,
        LineEndingStyle::Crlf => LineEnding::Crlf,
    }
    .as_str();

    let mut diagnostics = Vec::new();
    let doc = dialect.parse(&source, path, config, &mut diagnostics);
    debug_assert_eq!(doc.lines.len(), source.lines.len());
    let kept;
    let config = if doc.keep_keyword_case {
        let mut c = config.clone();
        let preprocessor = c.case.preprocessor;
        c.case = Default::default();
        c.case.preprocessor = preprocessor;
        kept = c;
        &kept
    } else {
        config
    };

    let rows = normalize::rows(&doc, &source.lines, config, dialect);
    let cols = layout::layout(&rows, config);
    let mut text = String::with_capacity(src.len() + src.len() / 4 + 16);
    text.push_str(source.bom_str());
    emit::emit(&rows, &cols, config, newline, &mut text);

    match verify::verify(dialect, src, &text) {
        Ok(()) => Formatted {
            text,
            diagnostics,
            check_failure: None,
        },
        Err(e) => Formatted {
            text: src.to_string(),
            diagnostics,
            check_failure: Some(e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::nasm::Nasm;

    fn fmt(src: &str) -> String {
        format(src, None, &Config::default(), &Nasm).text
    }

    #[test]
    fn trims_trailing_whitespace_and_final_blank_lines() {
        assert_eq!(fmt("mov eax, 1   \n\n\n"), "        mov eax, 1\n");
        assert_eq!(fmt("a\t\nb"), "a\nb\n");
        assert_eq!(fmt(""), "");
        assert_eq!(fmt("   \n\n"), "");
    }

    #[test]
    fn keeps_significant_trailing_whitespace() {
        assert_eq!(fmt("%define X 1 + \\  \n 2\n"), "%define X 1 + \\  \n 2\n");
    }

    #[test]
    fn line_endings() {
        assert_eq!(fmt("a\r\nb\n"), "a\r\nb\r\n");
        let lf = Config {
            line_ending: LineEndingStyle::Lf,
            ..Config::default()
        };
        assert_eq!(format("a\r\nb\r\n", None, &lf, &Nasm).text, "a\nb\n");
        let crlf = Config {
            line_ending: LineEndingStyle::Crlf,
            ..Config::default()
        };
        assert_eq!(format("a\nb\n", None, &crlf, &Nasm).text, "a\r\nb\r\n");
    }

    #[test]
    fn disabled_regions_are_byte_for_byte() {
        let src = "a  \n; reasm: off\nb  \n; reasm: on\nc  \n";
        assert_eq!(fmt(src), "a\n; reasm: off\nb  \n; reasm: on\nc\n");
    }

    #[test]
    fn reports_parse_diagnostics() {
        let out = format("db 'oops\n", None, &Config::default(), &Nasm);
        assert_eq!(out.diagnostics.len(), 1);
        assert_eq!(out.diagnostics[0].code, "W002");
    }

    #[test]
    fn keeps_bom() {
        assert_eq!(fmt("\u{feff}a  \n"), "\u{feff}a\n");
    }

    #[test]
    fn idempotent() {
        for src in ["a  \n\n", "a\r\n\r\nb", "%define X \\ \n", "\u{feff}\n\nx"] {
            let once = fmt(src);
            assert_eq!(fmt(&once), once, "input {src:?}");
        }
    }
}
