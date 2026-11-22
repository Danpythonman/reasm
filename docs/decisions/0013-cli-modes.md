# ADR-0013: Format paths in place by default; stdin → stdout when no paths

**Status:** Accepted (maintainer confirmed in-place writes, 2026-11-22). This also matches nasmfmt.

## Decision
`reasm file.asm` rewrites the file, like rustfmt and black. `reasm` with no
paths filters stdin to stdout, which is what editors need. `--stdout`, `--check`
and `--diff` change this. Writes are atomic (temp file + rename), and files
that didn't change are never touched.

## Alternatives
- **stdout by default and `-w` to write** (gofmt, clang-format): safer for a
  first run, but more typing in daily use.

## Consequences
Running it by accident on a directory modifies files. This is acceptable under
version control, and the safety check (ADR-0011) protects the content.
