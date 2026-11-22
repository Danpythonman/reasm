# Requirements

Keywords: **MUST** = required for 1.0. **SHOULD** = planned for 1.0, can slip
to a later release. **MAY** = nice to have, post-1.0 unless it turns out cheap.

Each requirement has an ID so that tests, ADRs and issues can reference it.

## 1. Correctness (highest priority)

| ID | Level | Requirement |
|---|---|---|
| COR-1 | MUST | Formatting MUST NOT change the assembled output. For any input that NASM assembles, `nasm` on the formatted file produces a byte-identical object file. |
| COR-2 | MUST | Formatting MUST be idempotent: `fmt(fmt(x)) == fmt(x)` for every config. |
| COR-3 | MUST | The formatter changes only whitespace, the case of tokens in known case-insensitive tables (ADR-0008), and explicitly enabled rewrites such as adding a colon to a label. Every other byte of every token is preserved. |
| COR-4 | MUST | Before writing a file, reasm checks that the input and output token streams are equivalent under COR-3 (ADR-0011). On a mismatch it does not write the file and reports an internal error. |
| COR-5 | MUST | String and character literals (`'..'`, `".."`, `` `..` ``) are never modified, including any `;` they contain. |
| COR-6 | MUST | A line that cannot be parsed with confidence is output verbatim (trailing whitespace trimmed) and a warning is printed (ADR-0006). |
| COR-7 | MUST | Symbol names, macro names, section names and numeric literals keep their original case. |
| COR-8 | MUST | The formatter never panics on any input, including invalid or binary data. Errors are reported, never crashes. |

## 2. Formatting features

| ID | Level | Requirement |
|---|---|---|
| FMT-1 | MUST | Labels (global, local `.x`, special `..@x`, macro-local `%%x`, context-local `%$x`) are placed at configurable columns. **Non-local and local labels have independent settings**, and both default to column 0 (ADR-0014). |
| FMT-2 | MUST | Instruction mnemonics are placed at a configurable indent. |
| FMT-3 | MUST | Operands are aligned in one of these modes: none (single gap), fixed column, or block-aligned (ADR-0010). |
| FMT-4 | MUST | Trailing comments are aligned in one of these modes: none, fixed column, or block-aligned. There is a minimum gap for lines that overflow. |
| FMT-5 | MUST | Comment-only lines are indented according to a configurable policy. |
| FMT-6 | MUST | Case of mnemonics, registers, size/operand keywords and directives is configurable separately (preserve / lower / upper). |
| FMT-7 | MUST | Comma spacing in operand lists is normalized. |
| FMT-8 | MUST | Spacing inside memory operands (`[...]`) is configurable: brackets, `+`/`-`, `*`. |
| FMT-9 | MUST | Consecutive blank lines are limited to a configurable maximum. Trailing whitespace is removed and there is exactly one final newline. |
| FMT-10 | MUST | Preprocessor and directive lines (`%define`, `section`, `global`, ...) have configurable indentation. |
| FMT-11 | MUST | Bodies of `%macro`, `%if*`, `%rep`, `struc`/`istruc` blocks are formatted. Each block kind has its own configurable extra indent per nesting level (ADR-0007). |
| FMT-12 | MUST | `; reasm: off` / `; reasm: on` regions and `; reasm: skip` (next line) are left completely untouched (ADR-0017). |
| FMT-13 | MUST | Lines with an inline label (`msg db "hi"`, `foo: mov eax, 1`) can be kept as they are or have the instruction split onto its own line, where this is legal (ADR-0019). |
| FMT-14 | SHOULD | Optionally add a colon to colonless labels. |
| FMT-15 | SHOULD | Alignment groups (the runs of lines whose columns are aligned together) have configurable break rules, and there is an outlier cap so one long line doesn't push the whole group out. |
| FMT-16 | MAY | Numeric literal normalization (hex digit case, prefix vs suffix style). |
| FMT-17 | MAY | Expression spacing outside memory operands. |

## 3. Configuration

| ID | Level | Requirement |
|---|---|---|
| CFG-1 | MUST | Options are read from `reasm.toml` or `.reasm.toml`, found by searching upward from the file being formatted (ADR-0005). |
| CFG-2 | MUST | Any option can be overridden on the command line with `--set key=value`. Precedence: defaults < file < CLI. |
| CFG-3 | MUST | Unknown keys and invalid values are errors, and the message names the key and its location. |
| CFG-4 | MUST | `reasm --print-config` prints the effective configuration as valid TOML. |
| CFG-5 | MUST | Dialect-specific options live in a dialect table (e.g. `[nasm]`). |
| CFG-6 | MUST | Every option is documented in CONFIGURATION.md with before/after examples. |

## 4. Command line

| ID | Level | Requirement |
|---|---|---|
| CLI-1 | MUST | Format files in place, format stdin to stdout, and walk directories for configured extensions. |
| CLI-2 | MUST | `--check` writes nothing, lists files that would change, and exits 1 if any would. |
| CLI-3 | MUST | `--stdout` prints formatted output instead of writing files. |
| CLI-4 | MUST | `--stdin-filepath` supports config discovery and diagnostics when the input comes from an editor. |
| CLI-5 | MUST | Exit codes are stable and documented (CLI.md). |
| CLI-6 | SHOULD | `--diff` prints a unified diff. |
| CLI-7 | SHOULD | `--verify` assembles the input and output with `nasm` and compares the object files (COR-1 at runtime). |

## 5. Extensibility

| ID | Level | Requirement |
|---|---|---|
| EXT-1 | MUST | Dialect-specific lexing, classification and keyword tables sit behind a `Dialect` trait. The layout engine, config, driver and diagnostics don't depend on any specific dialect (ADR-0004). |
| EXT-2 | MUST | The intermediate representation doesn't assume NASM conventions such as `;` comments or `%` preprocessing. |
| EXT-3 | MUST | Adding a dialect requires no changes to the layout engine. |
| EXT-4 | SHOULD | The formatter is usable as a library (`reasm::format_str`). |

## 6. Non-functional

| ID | Level | Requirement |
|---|---|---|
| NFR-1 | MUST | Single statically-linkable binary. The stripped release build targets **under 2 MB** on x86-64 Linux (budget raised from 1 MB to allow `clap`, ADR-0012). |
| NFR-2 | MUST | Formats 100,000 lines in under 100 ms on a modern desktop (single thread). |
| NFR-3 | MUST | Minimal runtime dependencies (ADR-0012). |
| NFR-4 | MUST | Builds on stable Rust, edition 2024, with `rust-version` declared. |
| NFR-5 | MUST | Runs on Linux, macOS and Windows. |
| NFR-6 | SHOULD | Diagnostics use the `file:line:col: severity: message` format so editors and CI can parse them. |

## 7. Non-goals

- **Line wrapping.** Assembly lines are not reflowed. There is no `max_width` option.
- **Semantic analysis.** No checking that instructions or operands are valid. That's NASM's job.
- **Following `%include`.** Each file is formatted on its own. Macros defined in included files can be declared in config (`nasm.known_macros`).
- **Expanding macros.** reasm never runs the preprocessor.
- **Rewriting code**, e.g. reordering, changing number bases (beyond FMT-16), or swapping instructions.
- **An LSP server** in 1.0. Editors integrate through stdin/stdout.
