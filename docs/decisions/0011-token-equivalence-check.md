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

## Implementation notes (M3)
- `Dialect::check_lines` returns tokens per line. For NASM, a preprocessor
  line is its name plus the rest as one token. A line whose keyword is a
  directive that is also a standard macro (`section`, `global`, `bits`, ...)
  is compared word by word at whitespace, because NASM splits directive
  arguments at whitespace (`section __LD,__compact_unwind` must not gain a
  space after the comma). Text that can't be lexed is one opaque token.
- Empty lines are ignored, and a line that is only `label:` is joined to the
  next one, so label splitting passes. An added colon is allowed only right
  after the first word of a physical line (`msg db` → `msg: db`), including
  the second line of a joined pair (`.local:` + `x equ 1` → `x: equ 1`).
- Known limit: a word that is a keyword somewhere (`add`) is allowed to change
  case anywhere, even where it is used as a symbol (`jmp add-2`). The
  classifier never transforms such operands, and the corpus run compares
  assembled output to cover this.
- During M3 the check blocked every formatter bug the corpus run found
  before any file was written: split `%%label:` lines, Mach-O section names,
  and smartalign's `alignmode`. One bug it missed (`add:` read as an
  instruction, then case-changed) was caught by comparing assembled output,
  which is why the corpus run assembles both versions. It also had one false
  positive of its own (a colon added on the second line of a joined pair),
  which was fixed.
- *Added in M4:* a check that uses the dialect's own lexer can't catch a
  lexer bug. x264's `v%{5}213%6` (one pasted name) was split by the lexer
  into two tokens and the formatter inserted a space between them; the
  token sequences still matched. The check now also rejects **whitespace
  inserted between tokens that touched in the input**, unless one side is
  punctuation the formatter spaces (`,` `[` `]` `(` `)` `:` `+` `-` `*`).
  Removing whitespace is still checked by re-lexing only.

## Alternatives
- **Running `nasm` every time:** this is the strongest check, but it needs
  nasm installed and the file to assemble on its own. It's available as
  `--verify` (CLI-7) instead.

## Consequences
Any formatter bug that would change tokens turns into a reported error
instead of silent corruption. The check doesn't catch reordering of whole
lines, but reasm never reorders lines.
