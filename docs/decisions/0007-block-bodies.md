# ADR-0007: Format macro/conditional bodies; per-kind nesting indent; `%if 0` stays verbatim

**Status:** Accepted (maintainer choice)

## Decision
- Lines inside `%macro`, `%imacro`, `%if*`, `%rep`, `struc` and `istruc` blocks
  are formatted like any other code.
- Each block kind has its own "extra indent per nesting level" option
  (`macro_body_indent`, `conditional_body_indent`, `rep_body_indent`,
  `struc_body_indent`). The extra indent is added to every column on the line,
  labels included.
- Defaults: macros 4, rep 4, conditionals 0 (they often wrap whole files), struc 0.
- `%if 0` … `%endif` bodies are verbatim because they're commonly used as
  block comments.
- Macro invocation arguments are kept as written unless
  `format_macro_args = true`.

## Consequences
The lexer must handle `%1`, `%%x`, `%{..}`, `%+` and so on in operand
positions. Anything it can't handle falls back to raw operands (ADR-0006).
