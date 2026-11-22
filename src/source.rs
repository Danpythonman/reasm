//! Source text: BOM handling, line splitting and line-ending detection
//! (ADR-0015, ADR-0016).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }
}

const BOM: char = '\u{feff}';

#[derive(Debug)]
pub struct Source<'a> {
    /// Whether the text started with a UTF-8 byte order mark (kept on output).
    pub bom: bool,
    /// The first line ending in the file, or LF if there is none.
    pub line_ending: LineEnding,
    /// Lines without their terminators. A final newline does not produce an
    /// extra empty line.
    pub lines: Vec<&'a str>,
}

impl<'a> Source<'a> {
    pub fn new(text: &'a str) -> Self {
        let (bom, body) = match text.strip_prefix(BOM) {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let line_ending = match body.find('\n') {
            Some(i) if body[..i].ends_with('\r') => LineEnding::Crlf,
            _ => LineEnding::Lf,
        };
        let body = body.strip_suffix('\n').unwrap_or(body);
        let lines = if body.is_empty() && !text.ends_with('\n') {
            Vec::new()
        } else {
            body.split('\n')
                .map(|l| l.strip_suffix('\r').unwrap_or(l))
                .collect()
        };
        Self {
            bom,
            line_ending,
            lines,
        }
    }

    pub fn bom_str(&self) -> &'static str {
        if self.bom { "\u{feff}" } else { "" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lines() {
        assert_eq!(Source::new("").lines, Vec::<&str>::new());
        assert_eq!(Source::new("\n").lines, vec![""]);
        assert_eq!(Source::new("a").lines, vec!["a"]);
        assert_eq!(Source::new("a\nb\n").lines, vec!["a", "b"]);
        assert_eq!(Source::new("a\r\nb\r\n").lines, vec!["a", "b"]);
        assert_eq!(Source::new("a\n\nb").lines, vec!["a", "", "b"]);
    }

    #[test]
    fn detects_line_endings() {
        assert_eq!(Source::new("a\nb\r\n").line_ending, LineEnding::Lf);
        assert_eq!(Source::new("a\r\nb\n").line_ending, LineEnding::Crlf);
        assert_eq!(Source::new("a").line_ending, LineEnding::Lf);
    }

    #[test]
    fn keeps_bom() {
        let s = Source::new("\u{feff}mov eax, 1\n");
        assert!(s.bom);
        assert_eq!(s.lines, vec!["mov eax, 1"]);
    }
}
