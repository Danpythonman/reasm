# ADR-0006: Unparseable lines are emitted verbatim with a warning

**Status:** Accepted (maintainer choice)

## Decision
Degradation happens one level at a time:
1. **Full:** every field is parsed and normalized.
2. **Raw operands:** the line's structure (label, keyword, comment) is clear
   but the operands can't be fully lexed. The fields are aligned and the
   operand text is kept as written. No warning, because this is expected
   inside macros.
3. **Verbatim line:** the structure is unclear (unterminated string, stray
   characters). Only trailing whitespace is trimmed, and warning `W001`/`W002`
   is printed with file:line:col.

Line-continuation groups (`\` at the end of a line) are verbatim without a
warning.

## Alternatives
- **Failing the whole file:** strict, but would make reasm unusable on real
  code full of macros. The maintainer rejected it.

## Consequences
The output may contain lines that don't follow the style. `--deny-warnings`
lets CI enforce full coverage when wanted.
