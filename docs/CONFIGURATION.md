# Configuration reference

reasm reads `reasm.toml` or `.reasm.toml` from the formatted file's directory,
or the nearest parent directory that has one (ADR-0005). Every key is optional.
A missing key takes its default. Unknown keys are errors.

You can override any key on the command line with its dotted path:

```sh
reasm --set labels.local_indent=4 --set case.mnemonic=upper src/
```

**The default style is nasmfmt's** (ADR-0020), with three intentional changes:
section directives and preprocessor lines (`%define`, `%macro`, ...) are not
indented, and local `.labels` stay in column 0 like every other label. Every other difference from nasmfmt fixes one of its bugs.
The full list is in ADR-0020.

Column numbers and indents in this document are **0-based**: column 0 is the
start of the line, and `indent.instruction = 8` means 8 spaces in front.

"Preserve" in spacing options means *keep whether there was a space, but shrink
runs of spaces to one*, which is what nasmfmt does.

## Default style at a glance

```nasm
%define SYS_WRITE 1
        global _start

section .text
; Starting point
_start:
        mov rax, 1                     ; write(fd, buf, len)
        mov rdi, 1
.loop:
        dec rcx
        jnz .loop

section .data
msg:
        db "Hello world!", 10
msglen  equ $-msg
```

---

## Top level

| Key | Type | Default | Description |
|---|---|---|---|
| `dialect` | string | `"auto"` | `"auto"` (chosen by file extension) or `"nasm"`. |
| `extensions` | array of strings | `["asm", "nasm", "inc"]` | File extensions picked up when walking directories. `inc` covers NASM include files. Remove it if your tree has non-NASM `.inc` files. Files named directly on the command line are always formatted. |
| `line_ending` | `"preserve"` \| `"lf"` \| `"crlf"` | `"preserve"` | ADR-0016. |

## `[indent]`

| Key | Type | Default | Description |
|---|---|---|---|
| `style` | `"spaces"` \| `"tabs"` | `"spaces"` | How leading indentation is written. Alignment inside a line always uses spaces (ADR-0015). |
| `tab_width` | integer 1–16 | `8` | Visual width of a tab, used to measure input and to produce tab indentation. |
| `instruction` | integer | `8` | Column of instructions and data keywords. Same as nasmfmt `-ii`. |

## `[labels]`

| Key | Type | Default | Description |
|---|---|---|---|
| `indent` | integer | `0` | Column of non-local labels (`foo:`). |
| `local_indent` | integer | `0` | Column of local labels (`.foo:`), plus special (`..@foo:`), macro-local (`%%foo:`) and context-local (`%$foo:`) labels. **This is the main change from nasmfmt.** |
| `split_inline` | `"preserve"` \| `"code"` \| `"all"` | `"all"` | `code`: move an instruction that shares a line with a label onto its own line. `all`: also split data lines (`msg db 1` → `msg:` / `db 1`). `equ` lines are **never** split, because `equ` needs its label on the same line. A word *without* a colon before an instruction (`foo nop`) is never split either: it could be a macro call (ADR-0019). |
| `add_colon` | bool | `false` | Add `:` to colonless labels that stay inline (`msg db 1` → `msg: db 1`). Labels moved to their own line by `split_inline` always get a colon. Like `split_inline`, it skips a colonless word before an instruction, and a label alone on its line. |
| `add_colon_to_equ` | bool | `false` | Also add the colon on `equ` lines (`x equ 1` → `x: equ 1`). |
| `inline_gap` | integer ≥ 1 | `1` | Minimum spaces between an inline label and the keyword after it. |

A label alone on its line *without* a colon (`foo`) always stays in column 0:
indented, reasm and readers would take it for a macro call.

```nasm
; input                     ; split_inline = "all" (default)   ; "preserve"
foo: mov eax, 1             foo:                               foo:    mov eax, 1
msg db "hi"                         mov eax, 1                 msg     db "hi"
len equ $-msg               msg:                               len     equ $-msg
                                    db "hi"
                            len     equ $-msg

; local_indent = 0 (default)        ; local_indent = 4
print:                              print:
.loop:                                  .loop:
        dec rcx                             dec rcx
```

## `[instructions]`

| Key | Type | Default | Description |
|---|---|---|---|
| `operand_align` | `"none"` \| `"fixed"` \| `"block"` | `"none"` | How the operand column is placed (ADR-0010). `none` = `min_gap` spaces after the mnemonic, like nasmfmt. |
| `operand_offset` | integer | `8` | With `fixed`: operands start this many columns after the mnemonic column. |
| `min_gap` | integer ≥ 1 | `1` | Minimum spaces between the mnemonic and its operands. |
| `inline_label_align` | `"none"` \| `"block"` | `"block"` | On lines that keep an inline label (always `equ` lines, plus everything under `split_inline = "preserve"`), `block` lines up the keyword column across the group, never earlier than `indent.instruction`. |

```nasm
; "none" (default)       ; "fixed", offset 8        ; "block"
        mov rax, 1               mov     rax, 1             mov   rax, 1
        movzx eax, al            movzx   eax, al            movzx eax, al
```

## `[operands]`

These rules apply to instructions, data, `equ` and `struc` lines when the
operand list could be fully lexed. Otherwise the operand text is kept exactly
as written (ARCHITECTURE §3). Strings are never touched. Directive arguments
(`section`, `global`, `[bits 64]`, ...) only have runs of spaces shortened,
because NASM splits them at whitespace (NASM.md §6). Outside `[...]`,
spacing around `+ - *` is kept as written (`expressions`). A space is never
removed where the two tokens would read back differently (`start %1`).

| Key | Type | Default | Description |
|---|---|---|---|
| `space_after_comma` | bool | `true` | Exactly one space after each comma (`false`: none). |
| `space_before_comma` | bool | `false` | Space before commas. |
| `bracket_padding` | `"preserve"` \| `"none"` \| `"padded"` | `"preserve"` | Spaces just inside `[...]`. |
| `memory_plus_minus` | `"preserve"` \| `"spaced"` \| `"compact"` | `"preserve"` | Binary `+`/`-` inside `[...]`. Unary minus is never spaced. |
| `memory_scale` | `"preserve"` \| `"spaced"` \| `"compact"` | `"preserve"` | `*` inside `[...]`. |
| `segment_colon` | `"preserve"` \| `"compact"` | `"preserve"` | Spaces around `:` in `fs:rax`, `[fs:0x28]`. |
| `size_keyword_space` | bool | `true` | Exactly one space after `byte`/`dword`/`strict`/`rel`/... |
| `expressions` | `"preserve"` | `"preserve"` | Expression spacing outside `[...]` (FMT-17, post-1.0). |

AVX-512 decorators (`{k1}`, `{z}`, `{1to16}`, `{rn-sae}`) always stay attached
to their operand.

## `[comments]`

| Key | Type | Default | Description |
|---|---|---|---|
| `align` | `"none"` \| `"fixed"` \| `"block"` | `"fixed"` | How trailing comments are placed. |
| `column` | integer | `39` | Column of the `;` for `fixed`, and the lowest column used by `block`. 39 matches nasmfmt's `-ci 40`, which counts from 1. Block indent (`[preprocessor] *_body_indent`) is not added to it. |
| `min_gap` | integer ≥ 1 | `1` | Minimum spaces between code and the comment when code reaches the column. |
| `standalone` | `"preserve"` \| `"code"` \| `"zero"` \| `"smart"` | `"zero"` | Indentation of comment-only lines. `zero` is column 0. `code` uses the column where the next code line starts (its label, or its keyword), or the previous code line's at the end of the file. `smart` keeps column-0 comments in column 0 and indents the rest like `code`. |
| `space_after_marker` | `"preserve"` \| `"ensure"` | `"ensure"` | `ensure` turns `;text` into `; text`. Banner lines (`;;`, `;-`, `;=`, `;*`, ...) and a bare `;` are never changed. |

## `[alignment]`

These only affect the `block` modes.

| Key | Type | Default | Description |
|---|---|---|---|
| `break_on` | array of strings | `["blank", "label", "block"]` | Events that end an alignment group: `"blank"` (blank line), `"label"` (a line that is only a label, including one made by `split_inline`), `"comment"` (comment-only line), `"block"` (`%macro`/`%if`/`%rep`/`struc` openers, closers, `%else`, and depth changes), `"directive"` (section, directive and `%` lines). A verbatim line always ends a group. |
| `max_column` | integer | `60` | A `block` column never goes past this. Rows that would need more use `min_gap` instead. |

## `[case]`

Values: `"preserve"` \| `"lower"` \| `"upper"`. Only tokens found in the
dialect's case-insensitive keyword tables are changed (ADR-0008). Like
nasmfmt, the default changes no case.

| Key | Default | Applies to |
|---|---|---|
| `mnemonic` | `"preserve"` | Instruction mnemonics and prefixes. |
| `register` | `"preserve"` | Registers. `$rax` is a symbol and is never changed. |
| `size_keyword` | `"preserve"` | `byte … zword`, and `strict near far short rel abs nosplit`. |
| `data` | `"preserve"` | `db … dz`, `resb … resz`, `times equ incbin align alignb`. |
| `directive` | `"preserve"` | `section global extern bits default org cpu ...`. |
| `preprocessor` | `"preserve"` | `%define %macro %if ...` (case-insensitive in NASM, verified with 2.16.01). |

## `[blank_lines]`

| Key | Type | Default | Description |
|---|---|---|---|
| `max_consecutive` | integer ≥ 0 | `1` | Longer runs of blank lines are shortened to this many. |
| `strip_leading` | bool | `false` | Remove blank lines at the start of the file. When `false`, the start of the file is still capped at `max_consecutive`. |

Trailing whitespace is always removed, and the file always ends with exactly
one newline.

## `[directives]`

| Key | Type | Default | Description |
|---|---|---|---|
| `section_indent` | `"zero"` \| `"instruction"` | `"zero"` | Indent of `section`, `segment`, `[section ...]` lines. **The second change from nasmfmt**, which indents them. |
| `indent` | `"zero"` \| `"instruction"` | `"instruction"` | Indent of other section-level directives: `global extern bits default cpu org common static required absolute`, plus output-format directives. |
| `arg_gap` | integer ≥ 1 | `1` | Spaces between the directive name and its arguments. |

## `[preprocessor]`

| Key | Type | Default | Description |
|---|---|---|---|
| `indent` | `"zero"` \| `"instruction"` \| `"nested"` | `"zero"` | Indent of `%` lines. `zero` = column 0 even inside blocks (the third change from nasmfmt, which uses `instruction`). `nested` = column 0 plus the current block indent. `instruction` = the instruction column plus the block indent. |
| `arg_gap` | integer ≥ 1 | `1` | Spaces between `%name` and its first argument. The rest of the line is kept as written. |
| `macro_body_indent` | integer | `0` | Extra indent per nesting level inside `%macro`/`%imacro`. |
| `conditional_body_indent` | integer | `0` | Extra indent per level inside `%if*` … `%endif`. |
| `rep_body_indent` | integer | `0` | Extra indent per level inside `%rep` … `%endrep`. |
| `struc_body_indent` | integer | `0` | Extra indent inside `struc`/`endstruc`, `istruc`/`iend`. |
| `format_macro_args` | bool | `false` | Apply `[operands]` rules to user macro calls. *Not implemented yet (M4); accepted and ignored.* |

Block indent is added to every column on the line, labels included, except
the trailing comment column and `%` lines with `indent = "zero"`. Lines
inside `%if 0` … `%endif` are always kept verbatim (ADR-0007). nasmfmt doesn't
handle nesting at all, which is why all body indents default to 0.

```nasm
; macro_body_indent = 4 (preprocessor.indent = "zero" is the default)
%macro PRINT 1
            mov rsi, %1       ; instruction 8 + body 4
    %%done:                   ; label 0 + body 4
%endmacro
```

## `[nasm]`

| Key | Type | Default | Description |
|---|---|---|---|
| `known_macros` | array of strings | `[]` | Macro names defined outside the file, used for label detection (ADR-0019). Case-sensitive. |
| `known_imacros` | array of strings | `[]` | Case-insensitive macros defined elsewhere. |

## `[diagnostics]`

| Key | Type | Default | Description |
|---|---|---|---|
| `allow` | array of strings | `[]` | Warning codes to silence, e.g. `["W001"]`. |
| `deny_warnings` | bool | `false` | Treat warnings as errors (exit 2). |

---

## Full default file

`reasm --print-config default` will print exactly this:

```toml
dialect = "auto"
extensions = ["asm", "nasm", "inc"]
line_ending = "preserve"

[indent]
style = "spaces"
tab_width = 8
instruction = 8

[labels]
indent = 0
local_indent = 0
split_inline = "all"
add_colon = false
add_colon_to_equ = false
inline_gap = 1

[instructions]
operand_align = "none"
operand_offset = 8
min_gap = 1
inline_label_align = "block"

[operands]
space_after_comma = true
space_before_comma = false
bracket_padding = "preserve"
memory_plus_minus = "preserve"
memory_scale = "preserve"
segment_colon = "preserve"
size_keyword_space = true
expressions = "preserve"

[comments]
align = "fixed"
column = 39
min_gap = 1
standalone = "zero"
space_after_marker = "ensure"

[alignment]
break_on = ["blank", "label", "block"]
max_column = 60

[case]
mnemonic = "preserve"
register = "preserve"
size_keyword = "preserve"
data = "preserve"
directive = "preserve"
preprocessor = "preserve"

[blank_lines]
max_consecutive = 1
strip_leading = false

[directives]
section_indent = "zero"
indent = "instruction"
arg_gap = 1

[preprocessor]
indent = "zero"
arg_gap = 1
macro_body_indent = 0
conditional_body_indent = 0
rep_body_indent = 0
struc_body_indent = 0
format_macro_args = false

[nasm]
known_macros = []
known_imacros = []

[diagnostics]
allow = []
deny_warnings = false
```
