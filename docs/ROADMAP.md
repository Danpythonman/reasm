# Roadmap

Every milestone ends with green tests and updated docs. Each one is a natural
point to stop and review.

| Milestone | Scope | Exit criteria |
|---|---|---|
| **M0: Planning** ✅ | This documentation | Maintainer has reviewed the ADRs and answered OPEN_QUESTIONS |
| **M1: Skeleton** ✅ | Cargo project, CLI (clap), config loading + `--set` + `--print-config`, driver, diagnostics. The formatter only trims trailing whitespace and normalizes newlines. | CLI tests pass; release binary built and size measured (1.28 MB stripped, 2026-11-22) |
| **M2: NASM front end** ✅ | Lexer, keyword tables + generator, classifier, block builder, verbatim fallback | Unit tests for every NASM.md item; NASM's own test suite (219 files, 28k lines) parses with 0 warnings |
| **M3: Layout** | Normalizer (case, operands), layout engine, emitter, safety check | Golden tests for `[indent] [labels] [instructions] [operands] [comments] [case] [alignment]` |
| **M4: Blocks & directives** | Preprocessor/macro/struc indentation, `%if 0`, `reasm: off/on/skip`, label rewrites | Golden tests for the remaining options |
| **M5: Hardening** | `--check`, `--diff`, `--verify`, assembly-equivalence tests, corpus run, benchmarks | Corpus formats with 0 safety failures; NFR-1/NFR-2 met |
| **1.0** | README examples, editor recipes, CI release binaries (Linux/macOS/Windows) | Tagged release |

## After 1.0

- Fuzzing in CI
- Numeric literal formatting (FMT-16), expression spacing (FMT-17)
- Range formatting for editors (`--lines 10:20`)
- Parallel file processing
- GAS dialect (Intel and AT&T syntax), then possibly MASM or other architectures' assemblers
- Optional `.gitignore`-aware directory walking
