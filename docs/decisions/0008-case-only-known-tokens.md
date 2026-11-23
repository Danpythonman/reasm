# ADR-0008: Case transforms only touch tokens in case-insensitive keyword tables

**Status:** Accepted

## Context
NASM symbols and macro names are case-sensitive. Changing the case of `MyLabel`
or a macro call `PRINT` would break the build. Mnemonics, registers, size
keywords and directives are case-insensitive.

## Decision
A token is case-transformed only if **both** of these hold:
1. The lexer/classifier put it in a transformable `TokenKind` (Mnemonic, Prefix,
   Register, SizeKeyword, OperandKeyword, Data, Directive,
   PreprocessorName), **and**
2. it matches an entry in the matching static table.

Identifiers, `$`-escaped names, macro calls, section names, strings and
numbers are never transformed. All of these keyword kinds were verified
case-insensitive with NASM 2.16.01 (NASM.md). `rip`, `eip` and `r8l` are
**not** NASM registers and stay symbols.

**Amended in M3:** a keyword is also left alone when some case variant of it
is a macro defined in the file or in `nasm.known_macros`/`known_imacros`
(`%define RAX rbx` makes `rax` → `RAX` unsafe, `%macro PUSH 1` makes
`push` → `PUSH` unsafe). The classifier demotes such tokens to identifiers
after its pre-pass.

*Added in M4:* the pre-pass also reads included files, and when any
macro name is computed (`%macro %1` in x264's `x86inc.asm`), no keyword
case is changed at all, since any keyword could be a macro (ADR-0022).

## Consequences
Missing table entries mean a keyword keeps its case, which is cosmetic. A
wrong extra entry could change the case of a symbol, so table additions need a
test. The safety check (ADR-0011) allows case differences only for these kinds.
