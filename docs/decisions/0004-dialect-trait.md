# ADR-0004: Dialects as a trait; only NASM in 1.0

**Status:** Accepted (maintainer request: modular, NASM only for now)

## Decision
A `Dialect` trait (ARCHITECTURE §4) provides parsing into the shared IR, the
keyword tables, the comment marker, its own config table, and the lexer used by
the safety check. Dialects are registered in a static slice. There are no
plugins or dynamic loading.

Each dialect may set its own defaults for shared options (core < dialect <
file < CLI).

## Alternatives
- **Enum dispatch (`match dialect {}`) throughout the code:** spreads dialect
  checks across the formatter. Rejected.
- **Dynamic plugins (WASM, shared libraries):** unnecessary, and they work
  against a single small binary.

## Consequences
The IR must stay dialect-neutral (EXT-2). Anything NASM-specific belongs in
`dialect/nasm`. Before 1.0, a short design sketch of GAS support should check
that the IR doesn't assume NASM. This is tracked in ROADMAP.
