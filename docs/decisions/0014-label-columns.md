# ADR-0014: Separate column settings for local and non-local labels; both default to 0

**Status:** Accepted (this is what motivated the project)

## Context
nasmfmt indents `.local:` labels while `global:` labels stay in column 0. The
maintainer wants all labels in column 0.

## Decision
`labels.indent` (non-local) and `labels.local_indent` (local, special,
macro-local, context-local) are independent integers, both defaulting to 0.
Block body indent (ADR-0007) is added on top inside macros and similar blocks.

## Consequences
Every label kind in NASM.md §3 gets a golden test for both settings.
