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

- *Added in M2, from tests against NASM 2.16.01 and its own test suite:*
  - Standard macros (`section`, `global`, `struc`, `align`, ...) are reserved
    even before a colon, because NASM expands them first (`section: db 1` is
    not a label). Plain directives (`list`, `map`) can be labels.
  - A lone word without a colon is a label only in column 0. When indented,
    it's treated as a macro call with no arguments. `add_colon` therefore
    never applies to lone words, because a wrong guess there could turn a
    macro call into a label.
  - Macro parameters and pasted names (`%1:`, `start%1:`, `.x_%+1:`) can be
    labels.

- *Added in M3, from running the formatter over NASM's test suite and
  comparing the assembled output:*
  - With a colon, instruction and data keywords **are** labels
    (`add: jmp add-2`, `do:`, `nop:`, `db:`, `equ:`; NASM's
    `test/insnlbl.asm`). Registers, size and special keywords, prefixes,
    `times` and standard macros are not. Verified with NASM 2.16.01.
  - A word without a colon before an **instruction or macro call** is still
    classified as a label, but it is never split off or given a colon,
    because it may be a macro from a `%use` package or an include file
    (`alignmode nop, 32` from `%use smartalign`). Before data keywords
    (`msg db 1`) it is always a label and may be split.
  - `%use smartalign` adds `alignmode` to the known macros.

- *Added in M4, from x264, dav1d and FFmpeg:*
  - A word before a keyword that is directly followed by a comma is a macro
    call, never a label: no statement keyword takes an empty first operand,
    so in `SBUTTERFLY dq, 0, 1, 2` (macro from an include) `dq` is an
    argument.

## Consequences
Code from projects that use many macros from `%include` files might want
`known_macros` set to get the best label detection. This is documented in
CONFIGURATION.md.
