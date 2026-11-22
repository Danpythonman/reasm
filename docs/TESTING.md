# Testing strategy

The goal is to show, with as much automation as possible, that reasm never
changes what the assembler produces (COR-1) and is idempotent (COR-2).

## 1. Layers

| Layer | Location | What it covers |
|---|---|---|
| Unit tests | `#[cfg(test)]` in each module | lexer edge cases, classifier decisions (NASM.md §4), layout column math, config parsing and errors |
| Golden tests | `tests/cases/<name>/` | end-to-end: `input.asm` + optional `reasm.toml` → `expected.asm` |
| Idempotency | automatic, every golden test | `format(expected) == expected` |
| Safety check | automatic, every golden test | the token-stream equivalence check (ADR-0011) passes |
| Assembly equivalence | `tests/assemble.rs` | for each golden input that assembles: `nasm -f elf64` on input and output, compare object bytes |
| Corpus | `tests/corpus/` (fetched, not vendored) | real-world NASM files, idempotency and assembly equivalence |
| Fuzzing | `fuzz/` (cargo-fuzz), post-1.0 | no panics, idempotency, safety check holds |
| CLI tests | `tests/cli.rs` | exit codes, `--check`, stdin mode, config discovery, `--set` |

## 2. Golden test format

```
tests/cases/local-labels-indented/
├── input.asm
├── expected.asm
└── reasm.toml        # optional; defaults if absent
```

A small custom test harness discovers cases. Setting `REASM_BLESS=1` rewrites
`expected.asm` files after you review them. This avoids a snapshot crate
dependency, though `insta` would be acceptable as a dev-dependency since it
doesn't affect binary size (see ADR-0012).

Each `CONFIGURATION.md` option gets at least one case for each value. Each
NASM.md item gets at least one case.

## 3. Assembly equivalence details

- The test assembles with `nasm -f elf64 -o out.o file.asm`, using the
  **same file name** in two separate temporary directories, because the source
  name ends up in the object file. `-f bin` is also tried for inputs that
  `elf64` rejects.
- The test is skipped (and reports that) when `nasm` isn't on PATH. In CI,
  nasm is installed so it always runs.
- Inputs that don't assemble (fragments, macro-only files) are marked with
  `; reasm-test: no-assemble` on the first line.

## 4. Corpus

`tools/fetch-corpus.sh` downloads pinned revisions of NASM-heavy projects into
`tests/corpus/` (git-ignored). The suite only runs with `--features corpus` or
`cargo test -- --ignored`. Candidates (licenses permit testing, not
redistribution):

- NASM's own `test/` directory
- x264 `common/x86/*.asm` and FFmpeg's `x86inc.asm`: heavy macro use, a good
  stress test
- small hobby OS kernels (bootloaders, `-f bin`)

## 5. Performance

`benches/`, or `tools/bench.sh` with `hyperfine`, formats a generated
100k-line file and checks NFR-2. Binary size (NFR-1) is checked in CI with
`stat` on the stripped release binary.
