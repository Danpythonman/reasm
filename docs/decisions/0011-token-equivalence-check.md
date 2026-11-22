# ADR-0011: Check token-stream equivalence before writing any file

**Status:** Accepted

## Context
COR-1 says formatting must never change the assembled output. Tests cover
known inputs, but users will run reasm on code no test has seen.

## Decision
After formatting, the driver re-lexes both the input and the output with the
dialect's `lex_check` (comments and strings count as single tokens) and
compares them. The only differences allowed are:
- whitespace,
- case, for tokens of transformable kinds (ADR-0008),
- an added `:` after a label when `add_colon` is on,
- comment text that differs only in the space after the marker
  (`space_after_marker`).

If anything else differs, the file isn't written, an internal error (exit 3)
is printed naming the first differing token, and other files go ahead. This
runs in release builds too. It costs one extra lex pass, which is cheap
compared to the risk.

## Alternatives
- **Running `nasm` every time:** this is the strongest check, but it needs
  nasm installed and the file to assemble on its own. It's available as
  `--verify` (CLI-7) instead.

## Consequences
Any formatter bug that would change tokens turns into a reported error
instead of silent corruption. The check doesn't catch reordering of whole
lines, but reasm never reorders lines.
