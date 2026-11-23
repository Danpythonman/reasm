//! The safety check (ADR-0011): the formatted text must have the same
//! tokens as the input, up to whitespace, keyword case, an added colon
//! after a label, and the spacing around comment text.

use crate::dialect::Dialect;
use crate::ir::{CheckKind, CheckLines, CheckToken};

/// A logical line: its 1-based number, its token range, and where in the
/// range its second physical line starts (when a `label:` line was joined).
type Logical = (usize, std::ops::Range<usize>, Option<usize>);

/// Non-empty lines. A line that is only `label:` is joined to the next
/// non-empty line, since splitting a label onto its own line
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
        let (number, joined, second) = match pending.take() {
            Some((n, r, _)) => (n, r.start..range.end, Some(2)),
            None => (i + 1, range, None),
        };
        if label_only && joined.len() == 2 {
            pending = Some((number, joined, None));
        } else {
            out.push((number, joined, second));
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
    for (k, ((na, ra, _), (nb, rb, second))) in a.iter().zip(&b).enumerate() {
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
                    i += 1;
                    j += 1;
                }
                // `msg db 1` → `msg: db 1` (labels.add_colon / split_inline):
                // a colon after the first word of a physical line.
                (_, Some(y))
                    if i == j
                        && y.kind == CheckKind::Colon
                        && tb[j - 1].kind == CheckKind::Word
                        && (j == 1 || Some(j - 1) == *second) =>
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
        let e = verify(&Nasm, "call Foo", "call foo").unwrap_err();
        assert!(e.contains("`Foo`") && e.contains("`foo`"), "{e}");
    }
}
