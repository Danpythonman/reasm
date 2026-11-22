# ADR-0015: UTF-8 input only; width counted in characters; tabs only for leading indentation

**Status:** Accepted

## Decision
- Input must be valid UTF-8. Anything else is a per-file error (exit 2). reasm
  never converts lossily. A UTF-8 BOM is preserved.
- Column width is the count of Unicode scalar values. East Asian wide
  characters in comments or strings may misalign later columns. That's
  documented and accepted, so the `unicode-width` dependency isn't needed.
- With `indent.style = "tabs"`, the leading indent is tabs (plus spaces for any
  remainder). Columns after the first cell are always padded with spaces, so
  alignment doesn't depend on the viewer's tab width.

## Consequences
Latin-1 encoded legacy sources must be converted first. If users ask for it, a
byte-level mode could be added later.
