# ADR-0003: Line-oriented IR, not a full expression AST

**Status:** Accepted

## Context
Formatters for languages like C or Rust build a full syntax tree because they
decide where to break lines. Assembly has no line wrapping (a non-goal), and
nearly all formatting decisions are about columns within a single line.

## Decision
Represent the file as `Vec<Line>`. Each line has an optional label, an optional
body (a keyword plus a token list) and an optional comment, plus block depth.
Operands are a flat token list, not an expression tree. Block structure
(`%macro`, `%if`, ...) is stored as a depth on each line, not as tree nesting.

## Alternatives
- **Full AST with expressions:** needed only for features like expression
  reformatting (FMT-17) or wrapping. Much more work, and every unsupported
  construct would become a parse failure.
- **Pure regex/string manipulation per line:** simpler, but there's no shared
  representation for a second dialect, and string and comment handling gets
  fragile.

## Consequences
Operand spacing rules are token-adjacency rules ("space after comma", "spaces
around binary + inside brackets"), which is enough for every FMT requirement.
If expression formatting is added later, the flat list can be parsed into a
tree locally without changing the line model.
