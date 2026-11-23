# NASM syntax notes

These are the NASM facts that reasm's lexer and classifier depend on, each with
tests in `src/dialect/nasm/{lexer,classify_tests}.rs`. **[verified]** marks
behavior confirmed by assembling small test files with NASM 2.16.01. Items with
no mark come from the manual or the NASM sources. Items marked **[verify]**
are still open.

References: the NASM manual, chapters 3 and 4, and the NASM 2.16.01 sources
that the keyword tables are generated from (ADR-0009).

## 1. Line structure

```
[label[:]] [prefixes] [instruction|pseudo-op|directive] [operands] [; comment]
```

- `;` starts a comment when it appears outside a string. NASM has no block
  comment syntax, and there's no statement separator.
- **Line continuation:** a line that ends in `\` continues on the next line.
  reasm keeps the whole group verbatim, with no warning (ADR-0006).
  - **[verified]** This also applies when the `\` ends a *comment*: in
    `db 1 ; note \`, the next line is part of the comment.
  - **[verified]** `\` followed by spaces is *not* a continuation, so trimming
    those spaces would change the program. reasm never trims whitespace after
    a trailing `\` (`Dialect::trailing_whitespace_significant`).
- Lines starting with `%` followed by a letter are preprocessor directives.
  `%%label`, `%$label` and `%1` start statements instead.
- A line wrapped in `[...]` is a primitive directive (`[bits 64]`,
  `[section .text]`).

## 2. Strings and characters

| Delimiter | Escapes |
|---|---|
| `'...'` | None. Ends at the next `'`. |
| `"..."` | None. Ends at the next `"`. |
| `` `...` `` | C-style backslash escapes. `` \` `` doesn't end the string. |

An unterminated string makes the line verbatim, with warning W002.

## 3. Identifiers and labels

- Valid characters: letters, digits, `_ $ # @ ~ . ?`. The first character must
  be a letter, `.`, `_` or `?`. A `?` on its own is the ternary operator.
- `$name` is a symbol even when `name` is a keyword (**[verified]**: `$rax:`
  is a label). `$` followed by a digit is a hex number (`$1F`). A bare `$` and
  `$$` are location tokens.
- Label kinds:
  | Form | Kind |
  |---|---|
  | `name` | non-local |
  | `.name` | local (attached to the previous non-local label) |
  | `..@name` | special: not attached, used in macros |
  | `..start`, `..imagebase`, ... | special symbols |
  | `%%name` | macro-local |
  | `%$name`, `%$$name` | context-local |
  | `%1`, `start%1`, `.x_%+1` | macro parameters and pasted names (in macro bodies) |
- **[verified]** With a colon, instruction and data keywords are labels:
  `add: jmp add-2`, `do:`, `nop:`, `db:`, `resb:`, `equ:` (NASM's
  `test/insnlbl.asm`). Registers, size keywords, `strict`/`rel`, prefixes
  (`lock`, `rep`, `wait`, segment registers), `times` and standard macros
  (`section`, `align`, `global`, `incbin`, ...) are not.
- A label alone on a line without a colon is legal **[verified]** (NASM warns
  with `orphan-labels`).
- **[verified]** Symbols and `%macro` names are case-sensitive. Instructions,
  registers, size keywords, directives, standard macros and preprocessor
  directive names are not (`%DEFINE`, `STRUC`, `ALIGN`, `SECTION`, `TIMES`,
  `DB` all assemble).
- **Token pasting [verified]:** in macro bodies, pieces like `start%1` or
  `.count_%+1` form a single name. The lexer joins an identifier and a `%`
  construct that touch with no space between them into one token, and digits
  and `{...}` groups that follow a `%` construct (`v%{5}213%6` from x264,
  `%%top_{%$v}%1` from NASM's `test/paste.asm`).

## 4. Classifying a line (ADR-0019)

reasm first makes a pre-pass that collects the names defined by `%macro`,
`%rmacro`, `%define`, `%xdefine`, `%assign`, `%defstr` and `%deftok`
(case-sensitive), and by their `%i...` forms (case-insensitive). `%use
smartalign` adds `alignmode`. It adds `nasm.known_macros` / `known_imacros`
from config. These names are used for label detection, and keywords that a
macro shadows in any case are never case-changed (ADR-0008).

Then, for each line:

1. Blank → `Blank`.
2. Split off the comment, respecting strings.
3. Starts with `%` + letter → `Preprocessor`. The name is matched against the
   generated directive table. Unknown names are kept but never case-changed.
4. Starts with `[` → bracketed `Directive` or `Section`. It must end with `]`.
5. Otherwise, lex the statement. Then decide whether it starts with a label:
   - `WORD :`, where `WORD` is an identifier or a macro-parameter form and is
     not a register, size or special keyword, prefix, `times` or **standard
     macro** (§3). **[verified]** `section: db 1` is the `section` macro, not
     a label, while `add:` and `list:` are labels.
   - `WORD` without a colon, followed by an instruction, prefix, data keyword,
     `equ`, `times`, struc keyword or known macro → colonless label
     (`msg db 1`, `list db 1` **[verified]**).
   - `WORD` alone: a label if it starts in column 0, otherwise a macro call
     without arguments.
6. Prefixes: `lock`, `rep*`, `o16`/`a32`/…, `{vex}`-style decorators, **segment
   registers** when an instruction follows (`es lodsb` **[verified]**), and
   `times <expr>` (the expression runs up to the next keyword). `wait` is both
   a prefix and an instruction: `wait fnop` vs `wait` alone **[verified]**.
7. Keyword → `Instruction`, `Data` (`db … dz`, `res*`, `incbin`, `align`,
   `alignb`), `Equ`, `Struc` (`struc endstruc istruc at iend`), `Section`,
   `Directive`, or `MacroCall`. An unknown word is assumed to be a macro from
   an included file, which gives note N001 with `--verbose`.
8. Operands are lexed into tokens, and identifiers are re-tagged as
   registers or keywords from the tables. Macro-call arguments and
   preprocessor bodies are kept as raw text. If the operands can't be lexed,
   they become raw text with no warning (ADR-0006 level 2).
9. If no structure can be found, the line is verbatim with warning W001.

Ambiguity: in `foo bar` where neither word is known, `foo` is treated as a
macro with argument `bar`. Both readings give the same layout and macro
arguments stay as written, so a wrong guess is only cosmetic.

## 5. Keyword tables

`src/dialect/nasm/tables/generated.rs` is generated from the NASM 2.16.01
sources by `tools/gen-tables/gen_tables.py` (ADR-0009):

| Table | Source | Entries |
|---|---|---|
| `MNEMONICS` | `x86/insnsn.c` (condition codes already expanded), minus data pseudo-ops | 2230 |
| `DATA_PSEUDO_OPS` | the same file: `db … dz`, `res*`, `incbin`, `equ` | 18 |
| `REGISTERS` | `x86/regs.dat`, ranges expanded | 248 |
| `PREFIXES` | `asm/tokens.dat` `TOKEN_PREFIX` (except `times`) | 24 |
| `SIZE_KEYWORDS` / `SPECIAL_KEYWORDS` | `asm/tokens.dat` | 8 / 9 |
| `DIRECTIVES` | `asm/directiv.dat` (global + output-format directives) | 27 |
| `PREPROCESSOR_DIRECTIVES` | `asm/pptok.dat`, expanded like `pptok.pl` | 119 |
| `STANDARD_MACROS` | `macros/standard.mac` (`%imacro` in the NASM section) | 28 |

There are hand-written groupings in `tables/mod.rs` for segment registers,
`align`/`alignb`, struc keywords and `section`/`segment`. A test checks they
are subsets of the generated tables.

**[verified]** These are **not** registers in NASM and stay symbols: `rip`,
`eip` (`lea rax, [rip+8]` fails with "symbol `rip` not defined"), `r8l`, and
`ptr`.

## 6. Operands

- Memory operand: `[size? seg: base + index*scale + disp]`, with optional
  `rel`/`abs`. `-` is unary when it follows `[`, `+`, `-`, `*`, `(`, `,`, `:`
  or begins the operand.
- AVX-512 decorators: `zmm1{k1}{z}`, `[rax]{1to16}`, `{rn-sae}`. Each `{...}`
  is one token.
- Operators (from `asm/tokens.dat`): `<<< >>> <=> << >> // %% == != <> <= >=
  && || ^^`, and the single-character operators `| ^ & ~ ! < > = / % + - *`
  and `?`. Whitespace is only removed between tokens when the concatenation
  re-lexes to the same tokens (the M3 safety check enforces this).
- Numbers are one token: digits, letters, `_` and `.`, plus a sign only right
  after an exponent marker (`e` in a decimal float, `p` in a hex float).
  **[verified]** `0x1e+5` is `0x1e` plus 5, and `1.5e+3` is a float.

- **[verified]** Directive arguments are split at whitespace, so spacing
  inside them is meaningful: `section __LD,__compact_unwind` (Mach-O) differs
  from `section __LD, __compact_unwind`. reasm only shortens runs of spaces
  in directive arguments; the `[operands]` rules don't apply to them.

- **[verified]** Macro arguments (ADR-0021): spacing around each argument
  is dropped, spacing inside is kept (one space per run) and reaches
  `%defstr`/`%str`; a greedy parameter (`1+`) keeps the commas and spacing
  of the rest of the line; `%ifidn` ignores spacing but not case;
  parentheses don't group arguments, braces do. Single-line macro
  arguments (`f( a ,b )`) also keep spacing and case through `%str`, and
  `f (x)` with a space is still a call.

## 7. Preprocessor

- **[verified]** Directive names are case-insensitive, so
  `case.preprocessor` can safely be `lower`/`upper`.
- Blocks:
  - `%macro`/`%imacro`/`%rmacro`/`%irmacro` … `%endmacro`/`%endm`
  - `%if*` … `%elif*`/`%else` … `%endif`
  - `%rep` … `%endrep`
  - `struc` … `endstruc` and `istruc` … `iend`

  `%elif`/`%else` sit at the opener's depth.
- `%if 0` … `%endif`: everything inside is verbatim. Nested `%if`s are
  counted, and an `%elif`/`%else` at the same level ends the region. Only the
  literal condition `0` triggers this.
- In macro bodies, `%1`, `%{10}`, `%+`, `%+1`, `%-1`, `%?`, `%??`, `%*?`,
  `%{1:-1}`, `%[expr]`, `%00` and `%!ENV` are single tokens.
- Unbalanced blocks give warning W003, either at the stray closer or at an
  opener that's never closed. Formatting continues with the best-effort depth.
