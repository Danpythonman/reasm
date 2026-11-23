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
NASM.md item gets at least one case. The harness is `tests/golden.rs`; it
also re-formats each output to check idempotency, and fails a case whose
safety check fails.

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

`tools/fetch-corpus.sh` downloads pinned revisions of x264, dav1d 1.5.5 and
FFmpeg 7.1.5 into `tests/corpus/` (git-ignored, about 12 MB: only the x86
asm directories are checked out). `tools/corpus/run.sh` runs
`tools/corpus/check_corpus.py` on each project with its include paths and
defines (stand-in `config.asm` files are in `tools/corpus/<project>/`),
under the default config plus any extra configs given as arguments.

`check_corpus.py` copies a tree, formats every `.asm`/`.inc`/`.mac` file
in the copy (so included files are formatted too), and checks the safety
check, idempotency, and that every file that assembles gives identical
object bytes in both trees. It exits 1 on any failure. Candidates
(licenses permit testing, not redistribution):

- NASM's own `test/` directory. Results at M2 (2026-11-22): 219 files and
  28,096 lines, 0 W001–W003 warnings, 0 panics. 4 deliberately corrupt fuzz
  files are rejected as invalid UTF-8. This run also found the token-pasting
  label case (`start%1:`), now covered by tests.
  Results at M3 (2026-11-23): formatted with the default config and two
  configs that turn on almost every option. 0 safety-check failures, 0
  idempotency failures, and all 189 files that assemble (`-f elf64` or
  `-f bin`) give byte-identical output before and after formatting. Getting
  there found and fixed: Mach-O section names (directive arguments are
  whitespace-separated), `%use smartalign`'s `alignmode`, and instruction
  names used as labels (`add:`).
- x264, dav1d and FFmpeg: heavy macro use through `x86inc.asm`. Results
  at M4 (2026-11-23): 229 files (x264 20, dav1d 49, FFmpeg 160) under the
  default config and two configs that turn on almost every option,
  `format_macro_args` included. 0 safety-check failures, 0 idempotency
  failures, and all 222 files that assemble for x86-64 give identical
  object files (the other 7 don't assemble with this setup, for example
  x264's 32-bit-only `dct-32.asm`). NASM's test suite, rerun
  under the same configs, is unchanged at 190/190. Getting there found
  and fixed:
  - a lexer bug in pasted names (`v%{5}213%6`, `%%top_{%$v}%1`), which the
    safety check missed because it uses the same lexer; the check now also
    rejects whitespace inserted between touching tokens (ADR-0011);
  - `SBUTTERFLY dq, 0, 1, 2` (macro from an include) split as a label plus
    `dq` (ADR-0019);
  - `{evex}vpmulhrsw` gaining a space, and a safety-check false positive
    when several labels are split onto their own lines;
  - case changes breaking x86inc's instruction macros (`%macro %1`,
    `RET`), fixed by reading included files and freezing keyword case
    when macro names can't all be known (ADR-0022);
  - `%str` in single-line macro calls (`f([ rax ])`) seeing changed
    spacing or case (ADR-0021).
- small hobby OS kernels (bootloaders, `-f bin`)

## 5. Performance

`benches/`, or `tools/bench.sh` with `hyperfine`, formats a generated
100k-line file and checks NFR-2. M3 numbers are in ARCHITECTURE §8: 94 ms for
100k lines of typical code, which meets NFR-2 with little margin. Long-line
input (69 bytes/line) takes 136 ms. Turning this into a CI check is M5 work. Binary size (NFR-1) is checked in CI with
`stat` on the stripped release binary.
