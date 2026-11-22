# ADR-0019: Colonless label detection and inline-label handling

**Status:** Accepted

## Context
NASM allows labels without colons (`msg db "hi"`, `len equ $-msg`, or a bare
`foo`). The line `foo bar` could be a label followed by an instruction, or a
macro `foo` with argument `bar`.

## Decision
- Use the classification algorithm in NASM.md §4. It relies on the keyword
  tables, macros defined in the file (pre-pass), and `nasm.known_macros`.
- When it's ambiguous, prefer **macro call**. Both readings give the same
  layout and macro arguments are kept as written, so a wrong guess is only
  cosmetic.
- `split_inline` values: `preserve`; `code` (split label + instruction or
  macro call); `all` (also split data lines like `msg db 1`, the default per
  ADR-0020, matching nasmfmt). A label moved onto its own line always gets a
  colon, so NASM doesn't print an orphan-label warning. `equ` lines are
  **never** split, because `equ` requires its label on the same line.
- `add_colon` never applies to `equ` lines unless `add_colon_to_equ` is set.

## Consequences
Code from projects that use many macros from `%include` files might want
`known_macros` set to get the best label detection. This is documented in
CONFIGURATION.md.
