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
| Assembly equivalence | `tests/assemble.rs` | for each golden case: `input.asm` and `expected.asm` assemble to identical bytes (22 of 36 cases at M5) |
| Corpus | `tests/corpus/` (fetched, not vendored) | real-world NASM files, idempotency and assembly equivalence |
| Fuzzing | `fuzz/` (cargo-fuzz), post-1.0 | no panics, idempotency, safety check holds |
| CLI tests | `tests/cli.rs` | exit codes, `--check`, `--diff`, `--verify`, stdin mode, config discovery, `--set` |

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

- The test uses the same code as `--verify` (`src/assemble.rs`,
  ADR-0023): `nasm -f elf64`, then `-f bin` for inputs that `elf64`
  rejects, with the **same file name** in two separate temporary
  directories, because the source name ends up in the object file.
- The test is skipped (and reports that) when `nasm` isn't on PATH. In CI,
  nasm is installed so it always runs.
- A case whose input doesn't assemble on its own (undefined symbols,
  fragments, deliberately broken lines) has a `no-assemble` file in its
  directory giving the reason. The test fails if a marked case does
  assemble, or an unmarked one doesn't, so the list stays accurate.

## 4. Corpus

`tools/fetch-corpus.sh` downloads pinned revisions of x264, dav1d 1.5.5,
FFmpeg 7.1.5 and NASM 2.16.01's `test/` directory into `tests/corpus/`
(git-ignored, about 15 MB: only the asm directories are checked out). `tools/corpus/run.sh` runs
`tools/corpus/check_corpus.py` on each project with its include paths and
defines (stand-in `config.asm` files are in `tools/corpus/<project>/`),
under the default config plus any extra configs given as arguments. The
two configs that turn on almost every option (all-upper and all-lower
case, every spacing and alignment option) are in
`tools/corpus/configs/`, and CI runs all three on every push.

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

`tools/check-nfr.py` checks the release binary's size (NFR-1, under
2 MiB) and times 15 whole-process runs on 100,000 lines built from the
golden inputs that assemble (NFR-2: median under 100 ms). Measured
numbers are in ARCHITECTURE §8; at M5 it's 1.54 MiB and 90 ms. CI runs it
with a 300 ms budget, because shared runners are slower and noisier; that
still catches large regressions.

## 6. Continuous integration

`.github/workflows/ci.yml` runs on every push to `main` and every pull
request:

| Job | What it runs |
|---|---|
| Test (Linux, macOS, Windows) | `cargo fmt --check` (Linux), `cargo clippy --all-targets`, `cargo test`, with nasm installed so the assembly tests run |
| Minimum Rust version | `cargo build` with the `rust-version` from `Cargo.toml` (NFR-4) |
| NFR | `tools/check-nfr.py --budget-ms 300` on the release build |
| Corpus | `tools/fetch-corpus.sh` (cached), then `tools/corpus/run.sh` with the default and both full configs |

`.gitattributes` marks `tests/cases/**` as binary, so Git on Windows
doesn't convert the line endings the golden tests compare.
