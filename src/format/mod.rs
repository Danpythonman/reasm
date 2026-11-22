//! The formatter core. Until M3 it only normalizes whitespace at line ends,
//! line endings and the end of the file, using the parse to keep
//! `reasm: off` regions byte for byte and to report diagnostics. The
//! normalizer, layout engine and safety check arrive in M3 (ROADMAP).

use crate::config::{Config, LineEndingStyle};
use crate::diag::Diagnostic;
use crate::dialect::Dialect;
use crate::ir::{LineContent, VerbatimReason};
use crate::source::{LineEnding, Source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formatted {
    pub text: String,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn format(src: &str, config: &Config, dialect: &dyn Dialect) -> Formatted {
    let source = Source::new(src);
    let newline = match config.line_ending {
        LineEndingStyle::Preserve => source.line_ending,
        LineEndingStyle::Lf => LineEnding::Lf,
        LineEndingStyle::Crlf => LineEnding::Crlf,
    }
    .as_str();

    let mut diagnostics = Vec::new();
    let doc = dialect.parse(&source, config, &mut diagnostics);
    debug_assert_eq!(doc.lines.len(), source.lines.len());

    // (text, may be dropped if blank at the end of the file)
    let mut lines: Vec<(&str, bool)> = source
        .lines
        .iter()
        .zip(&doc.lines)
        .map(|(&line, ir)| {
            if ir.content == LineContent::Verbatim(VerbatimReason::Disabled) {
                (line, false)
            } else if dialect.trailing_whitespace_significant(line) {
                (line, true)
            } else {
                (line.trim_end(), true)
            }
        })
        .collect();
    while lines
        .last()
        .is_some_and(|(l, droppable)| *droppable && l.is_empty())
    {
        lines.pop();
    }

    let mut text = String::with_capacity(src.len() + 1);
    text.push_str(source.bom_str());
    for (line, _) in &lines {
        text.push_str(line);
        text.push_str(newline);
    }
    Formatted { text, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::nasm::Nasm;

    fn fmt(src: &str) -> String {
        format(src, &Config::default(), &Nasm).text
    }

    #[test]
    fn trims_trailing_whitespace_and_final_blank_lines() {
        assert_eq!(fmt("mov eax, 1   \n\n\n"), "mov eax, 1\n");
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
        assert_eq!(format("a\r\nb\r\n", &lf, &Nasm).text, "a\nb\n");
        let crlf = Config {
            line_ending: LineEndingStyle::Crlf,
            ..Config::default()
        };
        assert_eq!(format("a\nb\n", &crlf, &Nasm).text, "a\r\nb\r\n");
    }

    #[test]
    fn disabled_regions_are_byte_for_byte() {
        let src = "a  \n; reasm: off\nb  \n; reasm: on\nc  \n";
        assert_eq!(fmt(src), "a\n; reasm: off\nb  \n; reasm: on\nc\n");
    }

    #[test]
    fn reports_parse_diagnostics() {
        let out = format("db 'oops\n", &Config::default(), &Nasm);
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
