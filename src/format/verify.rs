//! The safety check (ADR-0011): the formatted text must have the same
//! tokens as the input, up to whitespace, keyword case, an added colon
//! after a label, and the spacing around comment text.

use crate::dialect::Dialect;
use crate::ir::{CheckKind, CheckLines, CheckToken};

/// A logical line: its 1-based number, its token range, and where in the
/// range each physical line after the first starts (when `label:` lines
/// were joined to it).
type Logical = (usize, std::ops::Range<usize>, Vec<usize>);

/// Non-empty lines. Lines that are only `label:` are joined to the next
/// non-empty line, since splitting labels onto their own lines
/// (`labels.split_inline`) doesn't change the program. Empty lines have no
/// tokens, so a joined line is still one contiguous token range.
fn logical_lines(c: &CheckLines<'_>) -> Vec<Logical> {
    let mut out: Vec<Logical> = Vec::with_capacity(c.len());
    let mut pending: Option<Logical> = None;
    for i in 0..c.len() {
        let tokens = c.line(i);
        if tokens.is_empty() {
            continue;
        }
        let start = c.starts[i];
        let range = start..start + tokens.len();
        let label_only = tokens.len() == 2
            && tokens[0].kind == CheckKind::Word
            && tokens[1].kind == CheckKind::Colon;
        let line = match pending.take() {
            Some((n, r, mut seconds)) => {
                seconds.push(start - r.start);
                (n, r.start..range.end, seconds)
            }
            None => (i + 1, range, Vec::new()),
        };
        if label_only {
            pending = Some(line);
        } else {
            out.push(line);
        }
    }
    out.extend(pending);
    out
}

fn same(dialect: &dyn Dialect, a: &CheckToken, b: &CheckToken) -> bool {
    a.kind == b.kind
        && (a.text == b.text
            || (a.kind == CheckKind::Word
                && a.text.eq_ignore_ascii_case(b.text)
                && dialect.case_insensitive(a.text)))
}

/// Whether a space may be inserted between touching tokens `a` and `b`:
/// only next to punctuation the formatter spaces (`,` `[` `]` `(` `)`
/// `:` `+` `-` `*`). Elsewhere the two may be one token to the assembler
/// even if the dialect's lexer splits them (`v%{5}213%6`).
fn separable(a: &CheckToken, b: &CheckToken) -> bool {
    let punct =
        |c: Option<char>| matches!(c, Some(',' | '[' | ']' | '(' | ')' | ':' | '+' | '-' | '*'));
    punct(a.text.chars().last()) || punct(b.text.chars().next())
}

fn show(t: Option<&CheckToken>) -> String {
    match t {
        Some(t) if t.kind == CheckKind::Comment => format!("comment `;{}`", t.text),
        Some(t) => format!("`{}`", t.text),
        None => "end of line".into(),
    }
}

/// Returns a description of the first difference, if any.
pub fn verify(dialect: &dyn Dialect, input: &str, output: &str) -> Result<(), String> {
    let ca = dialect.check_lines(input);
    let cb = dialect.check_lines(output);
    let a = logical_lines(&ca);
    let b = logical_lines(&cb);
    for (k, ((na, ra, _), (nb, rb, seconds))) in a.iter().zip(&b).enumerate() {
        let ta = &ca.tokens[ra.clone()];
        let tb = &cb.tokens[rb.clone()];
        if ta == tb {
            continue;
        }
        let (mut i, mut j) = (0, 0);
        loop {
            match (ta.get(i), tb.get(j)) {
                (None, None) => break,
                (Some(x), Some(y)) if same(dialect, x, y) => {
                    if y.space_before && !x.space_before && !separable(&ta[i - 1], x) {
                        return Err(format!(
                            "safety check failed: input line {na} has `{}{}` where output \
                             line {nb} inserts a space",
                            ta[i - 1].text,
                            x.text
                        ));
                    }
                    i += 1;
                    j += 1;
                }
                // `msg db 1` → `msg: db 1` (labels.add_colon / split_inline):
                // a colon after the first word of a physical line.
                (_, Some(y))
                    if i == j
                        && y.kind == CheckKind::Colon
                        && tb[j - 1].kind == CheckKind::Word
                        && (j == 1 || seconds.contains(&(j - 1))) =>
                {
                    j += 1;
                }
                (x, y) => {
                    return Err(format!(
                        "safety check failed: input line {na} has {} where output line {nb} has {} \
                         (logical line {})",
                        show(x),
                        show(y),
                        k + 1
                    ));
                }
            }
        }
    }
    if a.len() != b.len() {
        let line = |v: &[Logical], n: usize| v.get(n).map_or(0, |l| l.0);
        let n = a.len().min(b.len());
        return Err(format!(
            "safety check failed: the input has {} code lines and the output {} \
             (first extra at input line {}, output line {})",
            a.len(),
            b.len(),
            line(&a, n),
            line(&b, n)
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::nasm::Nasm;

    fn ok(a: &str, b: &str) -> bool {
        verify(&Nasm, a, b).is_ok()
    }

    #[test]
    fn allowed_changes() {
        assert!(ok("mov  eax,1\n\n\n", "        mov eax, 1\n"));
        assert!(ok("MOV EAX, 1", "mov eax, 1"));
        assert!(ok("foo: mov eax, 1", "foo:\n        mov eax, 1"));
        assert!(ok("msg db 1", "msg:\n        db 1"));
        assert!(ok("msg db 1", "msg: db 1"));
        assert!(ok(".local:\nx equ 1", ".local:\nx: equ 1"));
        assert!(ok("nop ;x  ", "nop ; x"));
        assert!(ok("%DEFINE  X  1", "%define X  1"));
        assert!(ok("a:\nb: times 2 db 1", "a:\nb:\n        times 2 db 1"));
        assert!(ok("a:\nb:\nc db 1", "a:\nb:\nc:\n        db 1"));
    }

    #[test]
    fn rejected_changes() {
        // A symbol's case, a merged token, a changed string, a moved
        // token, a dropped line, a changed comment, and a removed colon.
        assert!(!ok("call Foo", "call foo"));
        assert!(!ok("dd a + b", "dd a+ b c"));
        assert!(!ok("db 'a, b'", "db 'a,b'"));
        assert!(!ok("mov eax, 1\nret", "mov eax,\n1 ret"));
        assert!(!ok("nop\nnop", "nop"));
        assert!(!ok("nop ; Hi", "nop ; hi"));
        assert!(!ok("foo: db 1", "foo db 1"));
        assert!(!ok("foo: mov es, x", "foo: mov: es, x"));
        assert!(!ok("%define X 1", "%define X  1"));
        assert!(!ok("v%{5}213%6 x", "v%{5} 213%6 x"));
        assert!(!ok("dd a<<b", "dd a << b"));
        assert!(ok("dd a+b", "dd a + b"));
        let e = verify(&Nasm, "call Foo", "call foo").unwrap_err();
        assert!(e.contains("`Foo`") && e.contains("`foo`"), "{e}");
    }
}
