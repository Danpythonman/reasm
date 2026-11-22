# ADR-0020: Default style is nasmfmt's, with three intentional changes

**Status:** Accepted (maintainer choice, 2026-11-22)

## Context
The maintainer wants nasmfmt's output as the reference style, except for three
things. The behavior below was taken from nasmfmt's `main.go`
and README (github.com/yamnikov-oleg/nasmfmt, master branch, read 2026-11-22).

## nasmfmt behavior adopted as defaults

| nasmfmt | reasm default |
|---|---|
| Instructions indented 8 spaces (`-ii 8`) | `indent.instruction = 8` |
| Single space between mnemonic and operands | `instructions.operand_align = "none"` |
| `, ` after commas, runs of spaces shrunk to one | `operands.space_after_comma = true`, "preserve" = shrink runs |
| `;` of trailing comments at column 40 (1-based), one space when the code is longer (`-ci 40`) | `comments.align = "fixed"`, `column = 39` (0-based), `min_gap = 1` |
| Comments rewritten as `; text` | `comments.space_after_marker = "ensure"` |
| Comment-only lines moved to column 0 | `comments.standalone = "zero"` |
| Labels on their own line at column 0; inline code moved to the next line | `labels.split_inline = "all"` |
| `msg db "x"` → `msg:` + `db "x"` | `split_inline = "all"`, with a colon added on split |
| No case changes | all `case.*` = `"preserve"` |
| Consecutive blank lines shrunk to one | `blank_lines.max_consecutive = 1`, `strip_leading = false` |
| `global`, `extern` etc. indented like instructions | `directives.indent = "instruction"` (maintainer confirmed keeping this) |
| No block nesting | all `*_body_indent = 0` |
| Writes files in place; `-` = stdin → stdout | ADR-0013 |

## Intentional changes (maintainer request)

1. **Local labels are never indented.** `.loop:` goes in column 0, like
   `_start:` (`labels.local_indent = 0`). nasmfmt's current source gives both
   label kinds the same treatment. The indentation the maintainer saw probably
   comes from a different version or fork, so reasm makes this a separate
   option either way (ADR-0014).
2. **Section directives are not indented.** `section .text` goes in column 0
   (`directives.section_indent = "zero"`), while `global`/`extern` stay
   indented like nasmfmt.
3. **Preprocessor lines are not indented.** `%define`, `%macro`, `%include`
   etc. go in column 0 (`preprocessor.indent = "zero"`). Lines inside a block
   can still be indented with the `*_body_indent` options.

## Differences where nasmfmt is wrong or loses information

These aren't style choices. reasm's correctness rules (COR-1..COR-5) require
them:

| nasmfmt | reasm |
|---|---|
| Comma and whitespace rules also apply inside strings (`db "a,b"` → `db "a, b"`), which changes the program | Strings are never modified |
| `equ` labels aren't recognized; the whole `msglen equ $-msg` line is indented | The label stays at column 0 and `equ` goes at the instruction column (`msglen  equ $-msg`) |
| `foo: db "x"` can produce `foo::` | Not possible: labels are parsed, not matched with regexes |
| `;; banner` → `; ; banner` | Banner comments are left alone |
| A bare `;` line becomes a blank line | It stays a comment |
| A space before a comma is kept (`rax , rbx`) | Removed (`space_before_comma = false`) |
| Data-label splitting only knows `db dw dd dq ddq do dt` | Uses the full data keyword table (`dy dz`, `res*`, `times`, `incbin`) |

## Consequences
The golden tests include nasmfmt's README example. Its expected output is
nasmfmt's output, adjusted only for the differences listed above. If a
nasmfmt-compatible preset is ever wanted, it's a config file. No code
changes are needed.
