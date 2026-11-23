//! Tokens for the safety check (ADR-0011) and the join test used when the
//! formatter removes a space (NASM.md §6).

use super::lexer;
use super::lexer::Tok;
use super::tables;
use crate::ir::{CheckKind, CheckLines, CheckToken, TokenKind};

/// Whether NASM reads `word` case-insensitively: it is in one of the
/// keyword tables, or a preprocessor directive name (ADR-0008).
pub fn case_insensitive(word: &str) -> bool {
    use tables::flag::*;
    if word.starts_with('%') {
        return tables::is(word, PREPROCESSOR_DIRECTIVES);
    }
    if word.starts_with('$') {
        return false;
    }
    tables::is(
        word,
        MNEMONICS
            | DATA_PSEUDO_OPS
            | REGISTERS
            | PREFIXES
            | SIZE_KEYWORDS
            | SPECIAL_KEYWORDS
            | DIRECTIVES
            | STANDARD_MACROS,
    ) || word.eq_ignore_ascii_case("times")
        || word.eq_ignore_ascii_case("equ")
}

pub fn joinable(a: &str, b: &str) -> bool {
    // Fast path: no NASM token or operator starts with `, ] )`, and none
    // continues after `, [ ] ( ) : + - *` (NASM.md §6; the exponent sign
    // in `1e+5` follows a letter, not one of these).
    let (Some(last), Some(first)) = (a.bytes().last(), b.bytes().next()) else {
        return true;
    };
    if matches!(first, b',' | b']' | b')')
        || (matches!(
            last,
            b',' | b'[' | b']' | b'(' | b')' | b':' | b'+' | b'-' | b'*'
        ) && a.len() == 1)
    {
        return true;
    }
    let joined = format!("{a}{b}");
    let (toks, err) = lexer::lex(&joined);
    err.is_none()
        && toks.len() == 2
        && toks[0].end == a.len()
        && toks[1].start == a.len()
        && toks[1].end == joined.len()
}

pub fn check_lines(text: &str) -> CheckLines<'_> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = CheckLines {
        tokens: Vec::with_capacity(text.len() / 6),
        starts: Vec::with_capacity(text.len() / 32),
    };
    let mut toks = Vec::new();
    for line in text.lines() {
        out.starts.push(out.tokens.len());
        check_line(line, &mut out.tokens, &mut toks);
    }
    out
}

fn other(text: &str) -> CheckToken<'_> {
    CheckToken {
        kind: CheckKind::Other,
        text,
        space_before: true,
    }
}

fn word(text: &str) -> CheckToken<'_> {
    CheckToken {
        kind: CheckKind::Word,
        text,
        space_before: true,
    }
}

fn check_line<'a>(line: &'a str, out: &mut Vec<CheckToken<'a>>, toks: &mut Vec<Tok>) {
    let (code, comment) = match lexer::split_comment(line) {
        Ok(split) => split,
        Err(_) => {
            let line = line.trim();
            if !line.is_empty() {
                out.push(other(line));
            }
            return;
        }
    };
    let code = code.trim();
    let bytes = code.as_bytes();
    if bytes.first() == Some(&b'%') && bytes.get(1).is_some_and(u8::is_ascii_alphabetic) {
        // Preprocessor line: the name, then the rest as written.
        let end = code[1..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .map_or(code.len(), |p| p + 1);
        let name = &code[..end];
        out.push(word(name));
        let rest = code[end..].trim();
        if !rest.is_empty() {
            out.push(other(rest));
        }
    } else if let Some((bracketed, name, rest)) = directive_line(code) {
        // NASM splits directive arguments at whitespace, so where
        // whitespace is must not change (only how much of it).
        if bracketed {
            out.push(other("["));
        }
        out.push(word(name));
        out.extend(rest.split_whitespace().map(word));
        if bracketed {
            out.push(other("]"));
        }
    } else if !code.is_empty() {
        let err = lexer::lex_into(code, toks);
        for (i, t) in toks.iter().enumerate() {
            let text = &code[t.start..t.end];
            let kind = match t.kind {
                TokenKind::Identifier | TokenKind::MacroParam => CheckKind::Word,
                TokenKind::Colon => CheckKind::Colon,
                _ => CheckKind::Other,
            };
            out.push(CheckToken {
                kind,
                text,
                space_before: i == 0 || t.space_before,
            });
        }
        if err.is_some() {
            let rest = code[toks.last().map_or(0, |t| t.end)..].trim();
            if !rest.is_empty() {
                out.push(other(rest));
            }
        }
    }
    if let Some(c) = comment {
        out.push(CheckToken {
            kind: CheckKind::Comment,
            text: c.trim(),
            space_before: true,
        });
    }
}

/// A line whose keyword is a directive that can't be a label (it is
/// also a standard macro): `section`, `global`, `bits`, ... Returns
/// (bracketed, name, arguments).
fn directive_line(code: &str) -> Option<(bool, &str, &str)> {
    let (bracketed, inner) = match code.strip_prefix('[') {
        Some(rest) => (true, rest.strip_suffix(']')?.trim()),
        None => (false, code),
    };
    let end = inner
        .find(|c: char| c.is_whitespace())
        .unwrap_or(inner.len());
    let name = &inner[..end];
    let directive = tables::is(name, tables::flag::DIRECTIVES)
        && tables::is(name, tables::flag::STANDARD_MACROS);
    directive.then_some((bracketed, name, &inner[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_test() {
        assert!(joinable("rax", "+"));
        assert!(joinable("[", "rax"));
        assert!(joinable("-", "1"));
        assert!(!joinable("foo", "bar"));
        assert!(!joinable("$", "foo"));
        assert!(!joinable("start", "%1"));
        assert!(!joinable("<", "<"));
        assert!(!joinable("1", "e"));
        assert!(!joinable("%", "[1]"));
        assert!(joinable("0x1e", "+"));
        assert!(joinable("1e", ","));
        assert!(joinable("*", "8"));
        assert!(joinable(":", "rax"));
    }

    #[test]
    fn lines() {
        let c = check_lines("foo: MOV eax, [rbx+1] ; hi \n%DEFINE  X  1\n\ndb 'oops\n");
        assert_eq!(c.len(), 4);
        let l: Vec<_> = (0..4).map(|i| c.line(i)).collect();
        let texts: Vec<_> = l[0].iter().map(|t| t.text).collect();
        assert_eq!(
            texts,
            [
                "foo", ":", "MOV", "eax", ",", "[", "rbx", "+", "1", "]", "hi"
            ]
        );
        assert_eq!(l[0][0].kind, CheckKind::Word);
        assert_eq!(l[0][10].kind, CheckKind::Comment);
        assert_eq!((l[1][0].kind, l[1][1].text), (CheckKind::Word, "X  1"));
        assert!(l[2].is_empty());
        assert_eq!(l[3], [other("db 'oops")]);
    }

    #[test]
    fn case_insensitive_words() {
        for w in [
            "MOV", "eax", "Times", "EQU", "%DEFINE", "section", "rel", "Db",
        ] {
            assert!(case_insensitive(w), "{w}");
        }
        for w in ["foo", "$rax", "%%x", "%foo", "rip", "MyFunc"] {
            assert!(!case_insensitive(w), "{w}");
        }
    }

    #[test]
    fn directive_arguments_keep_word_boundaries() {
        let a = check_lines("section __LD,__compact_unwind  data\n[ bits  64 ]");
        let b = check_lines("section __LD, __compact_unwind data\n[bits 64]");
        assert_eq!(a.line(0)[1].text, "__LD,__compact_unwind");
        assert_ne!(a.line(0), b.line(0));
        fn texts<'a>(c: &CheckLines<'a>) -> Vec<&'a str> {
            c.line(1).iter().map(|t| t.text).collect()
        }
        assert_eq!(texts(&a), texts(&b));
    }
}
