# ADR-0002: One package with lib + bin, organized into crate-shaped modules

**Status:** Accepted

## Context
The maintainer wants reasm to be modular, so new dialects can be added later.
A Cargo workspace with one crate per part (`reasm-core`, `reasm-nasm`,
`reasm-cli`) is the obvious way to enforce module boundaries.

## Decision
Start with a **single package** that has `src/lib.rs` and `src/main.rs`. The
modules (`ir`, `format`, `config`, `dialect::nasm`, ...) follow the dependency
rules in ARCHITECTURE §2. Split into a workspace when one of these happens:
1. a second dialect lands and compile times or features make it worthwhile, or
2. an outside consumer, such as an LSP or editor plugin, wants just the core.

## Alternatives
- **Workspace from day one:** enforces boundaries through the compiler, but
  adds Cargo and versioning overhead for a project with one dialect and no
  library users yet.

## Consequences
The boundaries are a convention until the split. Reviews (and an optional
`cargo-modules`/grep CI check) must enforce that `format` never imports
`dialect`. Splitting later is mostly mechanical because the module boundaries
already match the future crates.
