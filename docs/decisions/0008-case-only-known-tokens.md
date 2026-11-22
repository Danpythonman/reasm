# ADR-0008: Case transforms only touch tokens in case-insensitive keyword tables

**Status:** Accepted

## Context
NASM symbols and macro names are case-sensitive. Changing the case of `MyLabel`
or a macro call `PRINT` would break the build. Mnemonics, registers, size
keywords and directives are case-insensitive.

## Decision
A token is case-transformed only if **both** of these hold:
1. The lexer/classifier put it in a transformable `TokenKind` (Mnemonic, Prefix,
   Register, SizeKeyword, OperandKeyword, Data, Directive, and
   PreprocessorName once that's verified), **and**
2. it matches an entry in the matching static table.

Identifiers, `$`-escaped names, macro calls, section names, strings and
numbers are never transformed. Preprocessor directive case defaults to
`preserve` until its case-insensitivity is verified (NASM.md §7).

## Consequences
Missing table entries mean a keyword keeps its case, which is cosmetic. A
wrong extra entry could change the case of a symbol, so table additions need a
test. The safety check (ADR-0011) allows case differences only for these kinds.
