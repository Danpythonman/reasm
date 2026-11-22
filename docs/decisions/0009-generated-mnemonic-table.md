# ADR-0009: Keyword tables generated from the NASM sources and checked in

**Status:** Accepted

## Context
x86 has more than 1,500 mnemonics. The classifier needs them to tell
colonless labels apart (ADR-0019), and case transforms need them (ADR-0008).

## Decision
`tools/gen-tables/gen_tables.py` (Python, not part of the build) reads a
NASM source tree and writes `src/dialect/nasm/tables/generated.rs` with sorted
lowercase `&[&str]` tables. The sources are `x86/insnsn.c`, `x86/regs.dat`,
`asm/tokens.dat`, `asm/directiv.dat`, `asm/pptok.dat` and
`macros/standard.mac`.

*Amended in M2:* the plan was to read only `insns.dat`. The generated
`insnsn.c` was used instead because it already has condition codes expanded
(`jz`, `cmovnz`, ...). The other files replace hand-written lists for
registers, prefixes, directives and preprocessor directives, so all of them
match NASM exactly.

The generated file is committed with a header giving the NASM version
(2.16.01) and is marked `#[rustfmt::skip]`, so re-running the generator is
byte-for-byte reproducible. NASM is BSD-2-Clause licensed. Its license text
is in `THIRD_PARTY_NOTICES`.

## Alternatives
- **build.rs generation:** needs NASM sources at build time. Rejected.
- **The iced-x86 crate:** has mnemonic lists, but it's a big dependency for a
  string table.
- **No table, positional rules only:** can't tell `msg db 1` (label) from
  `PRINT msg` (macro).

## Consequences
New NASM instructions need the generator re-run on the new release's sources:
`python3 -I tools/gen-tables/gen_tables.py <nasm-src> > src/dialect/nasm/tables/generated.rs`. Missing entries degrade
gracefully: an unknown mnemonic is treated as a macro call, gets the same
layout, and keeps its case.
