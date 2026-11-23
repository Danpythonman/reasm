# ADR-0021: Respacing macro arguments

**Status:** Accepted (M4)

## Context
`preprocessor.format_macro_args` asks for the `[operands]` spacing rules on
user macro calls (`PRINT msg ,len` → `PRINT msg, len`). NASM substitutes
macro arguments as text, so it matters what NASM keeps of their spacing
and case. Verified with NASM 2.16.01 (`nasm -E`):

- Whitespace around each argument is dropped, so the spacing around the
  separating commas never reaches the expansion.
- Whitespace inside an argument is kept (one space per run). It only
  matters when the macro turns the argument into a string with
  `%defstr` or `%str` (`a + b` → `'a + b'`, `a+b` → `'a+b'`). `%ifidn`
  ignores it.
- A **greedy** last parameter (`%macro m 1+`) takes the rest of the line,
  commas and their spacing included.
- Case is kept, and matters: `%ifidn` is case-sensitive, and pasting
  (`%1_end`) turns `RAX` and `rax` into different symbols.
- Parentheses don't group arguments (`m (a,b)` has two). Braces do
  (`m {a, b}` has one).
- Arguments of **single-line** macros (`f(a, b)`, also in instruction
  operands) keep inner spacing and case through `%str`, and a space is
  allowed between the name and `(`.

## Decision
- The classifier splits a macro call's operands at top-level commas
  (`Operands::MacroArgs`). Braced groups are single tokens and stay as
  written.
- With `format_macro_args`, the operand spacing rules apply around commas
  and inside each argument. **Case is never changed** in macro arguments.
- The list is left exactly as written when:
  - the macro is defined in the file and its body contains `%defstr`,
    `%idefstr` or `%str` (also matched by `%strcat`/`%strlen`;
    conservative);
  - the arguments start with `(` (a single-line macro call);
  - the macro name is itself a parameter or pasted name (`%1 a, b`);
  - the operands don't lex.
- Arguments from the first greedy parameter on are kept as written, using
  the smallest greedy index over every `%macro` definition of the name.
- In **all** operands (not only macro calls), `name(...)` is treated as a
  single-line macro call: everything inside the parentheses keeps its
  spacing and case. This closes a gap where `bracket_padding` or
  `case.register` could change `%str` results.

## Consequences
- Macros defined in included files are not visible. A call to such a macro
  that stringifies its arguments, or has a greedy parameter, can produce a
  different string when `format_macro_args` is on. This is why the option
  is off by default; the corpus run (TESTING §4) checks real code with it
  on.
- The safety check (ADR-0011) compares tokens, so it doesn't catch spacing
  that reaches a string. The rules above are what prevent that.
