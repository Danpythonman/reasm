//! NASM lexer: comment splitting and tokenizing one line of code.
//! The rules follow docs/NASM.md §2, §3 and §6.

use crate::ir::TokenKind;

/// A token as byte offsets into the lexed text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tok {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
    pub space_before: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    /// Byte offset of the problem in the lexed text.
    pub offset: usize,
    pub message: String,
}

/// Splits a line at the first `;` outside a string.
/// Returns `(code, comment text after the marker)`.
pub fn split_comment(line: &str) -> Result<(&str, Option<&str>), LexError> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b';' => return Ok((&line[..i], Some(&line[i + 1..]))),
            q @ (b'\'' | b'"' | b'`') => i = skip_string(line, i, q)?,
            _ => i += 1,
        }
    }
    Ok((line, None))
}

/// Returns the offset just past the string starting at `start`.
/// Backquoted strings support backslash escapes; the others don't.
fn skip_string(text: &str, start: usize, quote: u8) -> Result<usize, LexError> {
    let bytes = text.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() {
        if quote == b'`' && bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            return Ok(i + 1);
        }
        i += 1;
    }
    Err(LexError {
        offset: start,
        message: format!("unterminated string starting with {}", quote as char),
    })
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || matches!(b, b'_' | b'.' | b'?')
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'#' | b'@' | b'~' | b'.' | b'?')
}

/// Multi-character operators, longest first (asm/tokens.dat).
const OPERATORS: &[&str] = &[
    "<<<", ">>>", "<=>", "<<", ">>", "//", "==", "!=", "<>", "<=", ">=", "&&", "||", "^^", "|",
    "^", "&", "~", "!", "<", ">", "=", "/",
];

/// Tokenizes `text`. On error, returns the tokens before the error too,
/// so the classifier can still use the start of the line (ADR-0006).
pub fn lex(text: &str) -> (Vec<Tok>, Option<LexError>) {
    let mut toks = Vec::new();
    let err = lex_into(text, &mut toks);
    (toks, err)
}

/// Like [`lex`], but appends to a reused buffer (cleared first).
pub fn lex_into(text: &str, toks: &mut Vec<Tok>) -> Option<LexError> {
    toks.clear();
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut space = false;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b' ' || b == b'\t' {
            space = true;
            i += 1;
            continue;
        }
        let start = i;
        let result = lex_one(text, i);
        match result {
            Ok((kind, end)) => {
                // Token pasting in macros: `start%1`, `.count_%+1`,
                // `%1_end` are single names. Pieces that touch are joined;
                // the result is a MacroParam if any piece is one.
                let pasteable = |k| matches!(k, TokenKind::Identifier | TokenKind::MacroParam);
                match toks.last_mut() {
                    Some(prev)
                        if !space
                            && prev.end == start
                            && pasteable(prev.kind)
                            && pasteable(kind) =>
                    {
                        prev.end = end;
                        if kind == TokenKind::MacroParam {
                            prev.kind = TokenKind::MacroParam;
                        }
                    }
                    _ => toks.push(Tok {
                        kind,
                        start,
                        end,
                        space_before: space,
                    }),
                }
                i = end;
                space = false;
            }
            Err(e) => return Some(e),
        }
    }
    None
}

fn take_while(bytes: &[u8], mut i: usize, f: impl Fn(u8) -> bool) -> usize {
    while i < bytes.len() && f(bytes[i]) {
        i += 1;
    }
    i
}

/// Finds the matching `close` for the `open` at `start`, allowing nesting.
fn matching(text: &str, start: usize, open: u8, close: u8) -> Result<usize, LexError> {
    let bytes = text.as_bytes();
    let mut depth = 0;
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Ok(i + 1);
                }
            }
            q @ (b'\'' | b'"' | b'`') => {
                i = skip_string(text, i, q)?;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    Err(LexError {
        offset: start,
        message: format!("unclosed `{}`", open as char),
    })
}

fn lex_one(text: &str, i: usize) -> Result<(TokenKind, usize), LexError> {
    use TokenKind as K;
    let bytes = text.as_bytes();
    let b = bytes[i];
    let next = bytes.get(i + 1).copied();
    let single = |kind| Ok((kind, i + 1));
    match b {
        b'\'' | b'"' | b'`' => Ok((K::String, skip_string(text, i, b)?)),
        b',' => single(K::Comma),
        b'[' => single(K::OpenBracket),
        b']' => single(K::CloseBracket),
        b'(' => single(K::OpenParen),
        b')' => single(K::CloseParen),
        b'+' => single(K::Plus),
        b'-' => single(K::Minus),
        b'*' => single(K::Star),
        b':' => single(K::Colon),
        b'{' => Ok((K::Decorator, matching(text, i, b'{', b'}')?)),
        b'0'..=b'9' => Ok((K::Number, number_end(bytes, i))),
        b'$' => match next {
            Some(n) if n.is_ascii_digit() => Ok((K::Number, number_end(bytes, i + 1))),
            Some(b'$') => Ok((K::Location, i + 2)),
            Some(n) if is_ident_start(n) => {
                Ok((K::Identifier, take_while(bytes, i + 1, is_ident_char)))
            }
            _ => single(K::Location),
        },
        b'%' => percent(text, i),
        b'?' if !next.is_some_and(is_ident_char) => single(K::Operator),
        b'.' if !next.is_some_and(is_ident_char) => Err(LexError {
            offset: i,
            message: "unexpected `.`".into(),
        }),
        _ if is_ident_start(b) => Ok((K::Identifier, take_while(bytes, i, is_ident_char))),
        _ => {
            for op in OPERATORS {
                if text[i..].starts_with(op) {
                    return Ok((K::Operator, i + op.len()));
                }
            }
            let ch = text[i..].chars().next().unwrap_or('?');
            Err(LexError {
                offset: i,
                message: format!("unexpected character `{ch}`"),
            })
        }
    }
}

/// Numbers: digits, letters (radix prefixes/suffixes, hex digits), `_`
/// and `.`; a `+`/`-` only directly after an exponent marker: `e` in a
/// decimal float (`1.5e+3`), `p` in a hex float (`0x1.8p+3`). So `0x1e+5`
/// is 0x1e plus 5, as NASM reads it (verified with NASM 2.16.01).
fn number_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    loop {
        i = take_while(bytes, i, |c| {
            c.is_ascii_alphanumeric() || c == b'_' || c == b'.'
        });
        let has_sign = matches!(bytes.get(i), Some(b'+' | b'-'));
        if !has_sign || i == start {
            return i;
        }
        let word = &bytes[start..i];
        let last = word[word.len() - 1].to_ascii_lowercase();
        let hex = word.len() > 2 && word[0] == b'0' && matches!(word[1] | 0x20, b'x' | b'h');
        let decimal = word[..word.len() - 1]
            .iter()
            .all(|c| c.is_ascii_digit() || *c == b'_' || *c == b'.');
        let exponent = (last == b'e' && decimal && !hex) || (last == b'p' && hex);
        if !exponent || !bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
            return i;
        }
        i += 1;
    }
}

/// `%` constructs outside preprocessor directive names (NASM.md §7).
fn percent(text: &str, i: usize) -> Result<(TokenKind, usize), LexError> {
    use TokenKind as K;
    let bytes = text.as_bytes();
    let at = |k: usize| bytes.get(i + k).copied();
    let param = |end| Ok((K::MacroParam, end));
    match at(1) {
        // %%name (macro-local) vs the %% operator (signed modulo).
        Some(b'%') if at(2).is_some_and(is_ident_char) => {
            param(take_while(bytes, i + 2, is_ident_char))
        }
        Some(b'%') => Ok((K::Operator, i + 2)),
        // %$name, %$$name (context-local).
        Some(b'$') => {
            let after = take_while(bytes, i + 1, |c| c == b'$');
            param(take_while(bytes, after, is_ident_char))
        }
        Some(b'{') => param(matching(text, i + 1, b'{', b'}')?),
        Some(b'[') => param(matching(text, i + 1, b'[', b']')?),
        Some(c) if c.is_ascii_digit() => param(take_while(bytes, i + 1, |c| c.is_ascii_digit())),
        // %+1 / %-1 (condition-code params) and %+ (token pasting).
        Some(b'+' | b'-') if at(2).is_some_and(|c| c.is_ascii_digit()) => {
            param(take_while(bytes, i + 2, |c| c.is_ascii_digit()))
        }
        Some(b'+') => param(i + 2),
        // %? %?? %*? %*??
        Some(b'?') => param(take_while(bytes, i + 1, |c| c == b'?')),
        Some(b'*') if at(2) == Some(b'?') => param(take_while(bytes, i + 2, |c| c == b'?')),
        // %!ENV
        Some(b'!') => param(take_while(bytes, i + 2, is_ident_char)),
        Some(c) if is_ident_start(c) => param(take_while(bytes, i + 1, is_ident_char)),
        _ => Ok((K::Operator, i + 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind as K;

    fn kinds(text: &str) -> Vec<(TokenKind, &str)> {
        let (toks, err) = lex(text);
        assert!(err.is_none(), "{text}: {err:?}");
        toks.iter()
            .map(|t| (t.kind, &text[t.start..t.end]))
            .collect()
    }

    #[test]
    fn comments_respect_strings() {
        assert_eq!(
            split_comment("mov eax, 1 ; hi").unwrap(),
            ("mov eax, 1 ", Some(" hi"))
        );
        assert_eq!(
            split_comment("db 'a;b' ; c").unwrap(),
            ("db 'a;b' ", Some(" c"))
        );
        assert_eq!(split_comment("db \"x;\", `\\`;` ;z").unwrap().1, Some("z"));
        assert_eq!(split_comment("nop").unwrap(), ("nop", None));
        assert!(split_comment("db 'oops ; no").is_err());
    }

    #[test]
    fn basic_instruction() {
        assert_eq!(
            kinds("mov rax, [rbx+rcx*4-8]"),
            [
                (K::Identifier, "mov"),
                (K::Identifier, "rax"),
                (K::Comma, ","),
                (K::OpenBracket, "["),
                (K::Identifier, "rbx"),
                (K::Plus, "+"),
                (K::Identifier, "rcx"),
                (K::Star, "*"),
                (K::Number, "4"),
                (K::Minus, "-"),
                (K::Number, "8"),
                (K::CloseBracket, "]"),
            ]
        );
    }

    #[test]
    fn space_before_is_recorded() {
        let (toks, _) = lex("a  b,c");
        let spaces: Vec<bool> = toks.iter().map(|t| t.space_before).collect();
        assert_eq!(spaces, [false, true, false, false]);
    }

    #[test]
    fn numbers() {
        for n in [
            "0x1F", "1Fh", "0b1010", "1010b", "777o", "1_000", "1.5e+3", "0x1.8p+3", "$1F", "3.14",
        ] {
            assert_eq!(kinds(n), [(K::Number, n)], "{n}");
        }
        assert_eq!(
            kinds("0x1e+5"),
            [(K::Number, "0x1e"), (K::Plus, "+"), (K::Number, "5")]
        );
        assert_eq!(kinds("1e+x")[0], (K::Number, "1e"));
    }

    #[test]
    fn identifiers_and_locations() {
        assert_eq!(kinds("$rax"), [(K::Identifier, "$rax")]);
        assert_eq!(kinds("$$"), [(K::Location, "$$")]);
        assert_eq!(
            kinds("$ - msg"),
            [(K::Location, "$"), (K::Minus, "-"), (K::Identifier, "msg")]
        );
        for id in [
            ".loop",
            "..@x",
            "..start",
            "?foo",
            "__?float32?__",
            "a#b@c~d",
        ] {
            assert_eq!(kinds(id), [(K::Identifier, id)], "{id}");
        }
        assert_eq!(kinds("a ? b : c")[1], (K::Operator, "?"));
    }

    #[test]
    fn macro_params() {
        for p in [
            "%1", "%10", "%%local", "%$ctx", "%$$outer", "%{1:-1}", "%[x+1]", "%+1", "%-1", "%?",
            "%??", "%*?", "%!HOME", "%0",
        ] {
            assert_eq!(kinds(p), [(K::MacroParam, p)], "{p}");
        }
        assert_eq!(
            kinds("j%+1 x"),
            [(K::MacroParam, "j%+1"), (K::Identifier, "x")]
        );
        assert_eq!(kinds("a %% b")[1], (K::Operator, "%%"));
        assert_eq!(kinds("a % b")[1], (K::Operator, "%"));
        assert_eq!(kinds("foo%+bar"), [(K::MacroParam, "foo%+bar")]);
        assert_eq!(
            kinds("start%1:"),
            [(K::MacroParam, "start%1"), (K::Colon, ":")]
        );
        assert_eq!(kinds("%1_end"), [(K::MacroParam, "%1_end")]);
        assert_eq!(kinds("a %1")[1], (K::MacroParam, "%1"));
    }

    #[test]
    fn decorators_and_operators() {
        assert_eq!(
            kinds("zmm1{k1}{z}")[1..],
            [(K::Decorator, "{k1}"), (K::Decorator, "{z}")]
        );
        assert_eq!(kinds("{rn-sae}"), [(K::Decorator, "{rn-sae}")]);
        assert_eq!(kinds("a<<<b")[1], (K::Operator, "<<<"));
        assert_eq!(kinds("a<=>b")[1], (K::Operator, "<=>"));
        assert_eq!(kinds("a//b")[1], (K::Operator, "//"));
    }

    #[test]
    fn errors_keep_earlier_tokens() {
        let (toks, err) = lex("mov eax, \\x");
        assert_eq!(toks.len(), 3);
        assert_eq!(err.unwrap().offset, 9);
        assert!(lex("{k1").1.is_some());
        assert!(lex("%{1").1.is_some());
        assert!(lex("db `a\\`").1.is_some());
    }
}
