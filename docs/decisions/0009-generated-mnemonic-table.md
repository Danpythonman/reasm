# ADR-0009: Mnemonic table generated from NASM's insns.dat and checked in

**Status:** Accepted

## Context
x86 has more than 1,500 mnemonics. The classifier needs them to tell
colonless labels apart (ADR-0019), and case transforms need them (ADR-0008).

## Decision
`tools/gen-tables` (a small Rust or Python script, not part of the build)
reads `x86/insns.dat` from a pinned NASM release (2.16.x) and writes
`src/dialect/nasm/tables/mnemonics.rs` as a sorted `&[&str]`. The generated
file is committed, with a header giving the NASM version and the BSD-2-Clause
notice. NASM is BSD-2-Clause licensed, and including derived data needs that
attribution, which also goes in `THIRD_PARTY_NOTICES`.

## Alternatives
- **build.rs generation:** needs NASM sources at build time. Rejected.
- **The iced-x86 crate:** has mnemonic lists, but it's a big dependency for a
  string table.
- **No table, positional rules only:** can't tell `msg db 1` (label) from
  `PRINT msg` (macro).

## Consequences
New NASM instructions need the generator re-run. Missing entries degrade
gracefully: an unknown mnemonic is treated as a macro call, gets the same
layout, and keeps its case.
