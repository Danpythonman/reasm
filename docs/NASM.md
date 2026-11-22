# NASM syntax notes

These are the NASM facts that reasm's lexer and classifier depend on. Each one
needs a golden test. Items marked **[verify]** must be confirmed against NASM
2.16 behavior (by assembling test files) before the code relies on them.

Reference: the NASM manual, chapter 3 ("The NASM Language") and chapter 4
("The NASM Preprocessor").

## 1. Line structure

```
[label[:]] [prefixes] [instruction|pseudo-op|directive] [operands] [; comment]
```

- `;` starts a comment when it appears outside a string. NASM has no block
  comment syntax, and there's no statement separator.
- **Line continuation:** a line ending in `\` continues on the next line. reasm
  keeps the whole continued group verbatim, with no warning (ADR-0006).
  **Verified (NASM 2.16.01):** `\` followed by spaces is *not* a continuation,
  so trimming those spaces would change the program. reasm never trims
  trailing whitespace after a `\` (`Dialect::trailing_whitespace_significant`).
- Lines starting with `%` are preprocessor directives, except `%%label`,
  `%$label` and `%1`-style parameters in statement positions.
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
  be a letter, `.`, `_` or `?`. **[verify]** the exact rules for `?` given
  NASM 2.15+ names like `__?NASM_VERSION_ID?__`.
- `$name` escapes a name that would otherwise be a keyword (`$eax` is a symbol).
  It is always an identifier and never changes case. A bare `$` and `$$` are
  location tokens.
- Label kinds:
  | Form | Kind |
  |---|---|
  | `name` | non-local |
  | `.name` | local (attached to the previous non-local label) |
  | `..@name` | special: not attached, used in macros |
  | `..start`, `..imagebase`, ... | special symbols (treated as non-local for layout) |
  | `%%name` | macro-local |
  | `%$name`, `%$$name` | context-local |
- A label alone on a line without a colon is legal (NASM warns with
  `orphan-labels`), so reasm treats it as a label too.
- Identifiers are case-sensitive. Instruction, register and keyword names are
  case-insensitive.

## 4. Classifying a line (ADR-0019)

Before classification, reasm makes a pre-pass that collects macro names from
`%macro NAME` (case-sensitive) and `%imacro NAME` (case-insensitive) in the
file, plus `nasm.known_macros` / `known_imacros` from config.

For each line (comment already split off):

1. Blank → `Blank`.
2. Starts with `[` and ends with `]` → bracketed `Directive`.
3. First token is a `%` directive name → `Preprocessor`.
4. First token is an identifier or label form followed by `:` → label with a
   colon. The rest of the line is classified from step 5 on.
5. First token is a prefix or mnemonic → `Instruction`.
6. First token is a data keyword (`db … dz`, `resb … resz`, `incbin`, `equ`,
   `times`, `align`, `alignb`) → `DataOrEqu`.
7. First token is a section-level directive → `Directive`.
8. First token is a known macro → `MacroCall`.
9. First token is an identifier and **either** nothing follows, **or** the next
   token is a mnemonic, prefix, data keyword or known macro → colonless label.
   Classification continues from step 5 on the rest.
10. First token is an identifier and something else follows → `MacroCall`, on
    the assumption it's a macro from an included file. This prints a note with
    `--verbose` but no warning.
11. Anything else → `Verbatim`, with warning W001.

Ambiguity: `foo bar` where neither word is known. Step 10 treats `foo` as a
macro called with argument `bar`. The layout is the same either way (`foo` at
the mnemonic column), and because macro arguments are kept as written, getting
this wrong is cosmetic. It never changes the meaning.

## 5. Keyword tables

| Table | Source | Notes |
|---|---|---|
| Mnemonics | generated from NASM's `x86/insns.dat` (ADR-0009) | includes pseudo-ops recognized as instructions |
| Prefixes | hand-written | `lock rep repe repz repne repnz xacquire xrelease bnd nobnd o16 o32 o64 a16 a32 a64 wait`, plus `{vex}`, `{vex2}`, `{vex3}`, `{evex}` |
| Registers | hand-written | GPRs (all sizes, `r8b`/`r8l` forms), segment, `cr*`, `dr*`, `tr*`, `st0-7`, `mm0-7`, `xmm/ymm/zmm0-31`, `k0-7`, `bnd0-3`, `tmm0-7`, `rip`/`eip` |
| Size/operand keywords | hand-written | `byte word dword qword tword oword yword zword strict near far short to rel abs nosplit` |
| Data keywords | hand-written | `db dw dd dq dt do dy dz resb resw resd resq rest reso resy resz incbin equ times align alignb` |
| Directives | hand-written | `bits use16 use32 use64 default section segment absolute extern required global static common cpu float org warning map` and output-format directives (`import export group uppercase library module safeseh osabi`) **[verify]** the list for 2.16 |
| Struct macros | hand-written | `struc endstruc istruc at iend`. Opening/closing these sets the block for `struc_body_indent`. |

All tables are sorted `&'static [&'static str]` and looked up with ASCII
case-insensitive binary search.

## 6. Operands

- Memory operand: `[size? seg: base + index*scale + disp]`, with optional
  `rel`/`abs`, e.g. `[rel msg]`. `-` is unary when it follows `[`, `+`, `-`,
  `*`, `(`, `,`, `:` or begins the operand.
- AVX-512: `zmm1{k1}{z}`, `[rax]{1to16}`, `, {rn-sae}`. Braces always stay
  attached.
- Operators NASM recognizes: `| ^ & << >> <<< >>> + - * / // % %% ~ ! ( )`,
  comparisons, `? :`. **[verify]** the full list. Rule: whitespace is only
  removed between tokens when the concatenation re-lexes to the same tokens.
  The safety check enforces this.
- Floats (`1.5e3`, `0x1.8p3`), special floats (`__?Infinity?__`), and numbers
  with `_` separators or prefix/suffix radix (`0x1F`, `1Fh`, `$1F`?
  **[verify]** whether `$`-prefixed hex is still accepted) are lexed as a single
  `Number` token and copied unchanged.

## 7. Preprocessor

- Directive names: `%define %xdefine %idefine %undef %assign %iassign %strlen
  %substr %macro %imacro %endmacro %rotate %rep %endrep %exitrep %if* %elif*
  %else %endif %include %pathsearch %depend %use %push %pop %repl %error
  %warning %fatal %pragma %line %clear %arg %stacksize %local`, and the 2.15+
  additions. **[verify]** the list. **[verify]** case-insensitivity: if `%DEFINE`
  is accepted, `case.preprocessor` can safely default to `lower`.
- Block openers/closers: `%macro`/`%imacro` … `%endmacro`; `%if*` … `%elif*`
  / `%else` … `%endif`; `%rep` … `%endrep`. `%elif`/`%else` are **middle**
  lines: they sit at the opener's depth.
- `%if 0` … `%endif`: everything inside is verbatim. Only the literal
  condition `0` triggers this. reasm doesn't evaluate other conditions, so
  `%if 1` … `%else` bodies are formatted normally.
- Inside macro bodies: `%1`–`%9`, `%{10}`, `%+`, `%?`, `%??`, `%{1:-1}`,
  `%[expr]`, `%00`. These lex as `MacroParam`/`Other`. If a construct can't be
  lexed, the operands become `Raw`, which is aligned but kept as written.
- Unbalanced blocks (e.g. `%endmacro` without an opener) produce warning W003.
  Depth stays at 0 and formatting continues.
