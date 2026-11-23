# Architecture

## 1. Overview

reasm is a line-oriented formatter. Each file goes through a fixed pipeline. Only
the first two stages depend on the dialect. Everything after the IR is shared.

```
                ┌──────────────── dialect (NASM) ───────────────┐
 bytes ─► Source ─► Lexer ─► Classifier/Block builder ─► Document (IR)
                └───────────────────────────────────────────────┘
                                                             │
                ┌──────────────────── core ──────────────────▼───────────┐
                │ Normalizer (case, operand spacing, label rewrites)     │
                │   ─► Layout engine (cells, alignment groups, columns)  │
                │   ─► Emitter (string + line endings)                   │
                │   ─► Safety check (token-stream equivalence, ADR-0011) │
                └────────────────────────────────────────────────────────┘
                                                             │
                                          Driver ─► write / --check / --diff
```

## 2. Package layout

There's one Cargo package with a library and a binary target (ADR-0002). The
modules are arranged so each top-level module could become its own crate later
without changing its API.

```
reasm/
├── Cargo.toml
├── src/
│   ├── main.rs              # binary: argument parsing, calls cli::run
│   ├── lib.rs               # public API: format_str, Config, Dialect registry
│   ├── cli/                 # argument parsing (clap derive), modes, exit codes
│   ├── driver/              # file discovery, config resolution per file, I/O, check/diff
│   ├── config/              # option structs (incl. each dialect's table, e.g. [nasm]), TOML loading, --set, discovery
│   ├── diag/                # Diagnostic, Severity, Span, rendering
│   ├── source/              # Source text, line splitting, line-ending detection
│   ├── ir/                  # Document, Line, Label, Body, Token, TokenKind, Block
│   ├── format/
│   │   ├── normalize.rs     # case transforms, operand spacing, colon insertion
│   │   ├── layout.rs        # cells → alignment groups → column positions
│   │   ├── emit.rs          # final text
│   │   └── verify.rs        # token-stream equivalence check
│   └── dialect/
│       ├── mod.rs           # Dialect trait, registry, selection by name/extension
│       └── nasm/
│           ├── mod.rs       # impl Dialect for Nasm
│           ├── lexer.rs     # comment splitting, tokens, token pasting
│           ├── classify.rs  # line → Line IR, label detection, block structure,
│           │                # %if 0, reasm: off/on/skip (tests in classify_tests.rs)
│           └── tables/      # generated.rs (from NASM sources) + hand-written groupings
├── tools/gen-tables/        # gen_tables.py: keyword tables from the NASM sources (ADR-0009)
└── tests/
    ├── cases/               # golden tests (input.asm, expected.asm, reasm.toml)
    └── ...
```

Dependency direction (an arrow means "may use"):

```
cli → driver → { config, dialect, format, diag, source }
dialect → { ir, diag, source, config (reads its own table only) }
config  → dialect (only to validate the `dialect` name)

Dialect option structs (`NasmOptions`) live in `config` rather than in each
dialect module. `deny_unknown_fields` needs the complete `Config` type, and
keeping it in one place avoids a config ↔ dialect dependency cycle.
format  → { ir, config, diag }
ir, diag, source → (nothing internal)
```

`format` never imports `dialect`. Anything the formatter needs to know about a
dialect is carried in the IR (EXT-1, EXT-3).

## 3. Data model (IR)

The IR models one physical line per entry. Assembly is line-structured, so a
full expression AST isn't needed (ADR-0003).

```rust
struct Document {
    lines: Vec<Line>,
    line_ending: LineEnding,        // detected, ADR-0016
}

struct Line {
    number: u32,                    // 1-based source line number
    depth: BlockDepth,              // nesting per block kind (macro/cond/rep/struc)
    content: LineContent,
}

enum LineContent {
    Blank,
    Verbatim(VerbatimReason),       // Unparsed | Continuation | Disabled (reasm: off) | CommentedOut (%if 0)
    Code(CodeLine),
}

struct CodeLine {
    label:   Option<Label>,
    body:    Option<Body>,
    comment: Option<Comment>,
    original_column: u32,           // visual column where the line's content started
}

struct Label {
    name: String,                   // exact original text
    kind: LabelKind,                // NonLocal | Local | Special(..@) | MacroLocal(%%) | ContextLocal(%$)
    colon: bool,
}

struct Body {
    kind: BodyKind,       // Instruction | Data | Equ | Section | Directive | Struc | Preprocessor | MacroCall
    prefixes: Vec<Token>, // lock, rep, es, {vex}, `times <expr>`
    keyword: Token,       // mnemonic, data keyword, directive, %name or macro name
    operands: Operands,
    role: BlockRole,      // None | Open(kind) | Middle(kind) | Close(kind)
    bracketed: bool,      // [bits 64] form
}

enum Operands {
    Tokens(Vec<Token>),             // fully lexed: spacing may be normalized
    Raw(RawText),                   // couldn't lex fully: aligned as a unit, text untouched
}

struct Token { kind: TokenKind, text: String, space_before: bool }

enum TokenKind {
    Mnemonic, Prefix, Register, SizeKeyword, OperandKeyword, // case-transformable categories
    Directive, PreprocessorName,
    Identifier, Number, String, MacroParam,                  // never case-transformed
    Comma, OpenBracket, CloseBracket, OpenBrace, CloseBrace,
    Plus, Minus, Star, Colon, Operator, Other,
}

struct Comment { marker: String, text: String }  // marker is ";" for NASM, "#" or "//" for others (EXT-2)
```

Notes:

- `Token.text` always holds the original text. Case transforms create new text
  only for case-transformable kinds (ADR-0008).
- `Operands::Raw` lets reasm degrade one field at a time. A line it can't fully
  lex inside the operands (for example exotic `%[...]` preprocessor
  constructs) is still aligned, but its operand text is kept as is. Only a line
  whose structure itself is unclear becomes `Verbatim` and triggers a warning.
- `BlockDepth` counts nesting per block kind, so each kind can have its own
  indent (FMT-11).

## 4. Dialect abstraction

```rust
pub trait Dialect {
    fn name(&self) -> &'static str;                  // "nasm"
    fn extensions(&self) -> &'static [&'static str]; // ["asm", "nasm", "inc"]
    fn trailing_whitespace_significant(&self, line: &str) -> bool;
    fn parse(&self, src: &Source, config: &Config, diags: &mut Vec<Diagnostic>) -> Document;
    /// Can tokens `a` and `b` be written with no space between them and
    /// still read back as the same two tokens? (operand spacing)
    fn joinable(&self, a: &str, b: &str) -> bool;
    /// Per-line tokens for the safety check (ADR-0011).
    fn check_lines(&self, text: &str) -> Vec<Vec<CheckToken>>;
}
```

- The **registry** is a static list: `&[&Nasm]`. A dialect is chosen by
  `--dialect`, then by the config `dialect` key, then by file extension, with
  `nasm` as the fallback.
- **What a dialect provides:** lexing, line classification, block structure,
  keyword tables, comment marker, its config table, and a lexer used by the
  safety check.
- **What core provides:** the IR, normalization, layout, emitting, safety
  check, config loading, CLI and diagnostics.
- **Adding GAS later** means a `dialect/gas/` module plus a `[gas]` config
  table. If GAS needs a new IR concept, such as `#` comments or `%`-prefixed
  registers, it goes into `TokenKind`/`Comment` in a dialect-neutral form.

Each dialect may provide its own default values for shared options. For example,
GAS users conventionally indent with a tab. The precedence order becomes
core default < dialect default < file < CLI.

## 5. Formatting stages

### 5.1 Normalizer

`format/normalize.rs` turns each IR line into a `Row`: `Blank`, `Verbatim`
(text as written), `Comment` (a comment-only line), or `Code` with up to
four rendered cells and each cell's *base* column (label indent, keyword
indent by kind, block indent). On the way it applies:

1. Case transforms for transformable token kinds (`[case]`).
2. Operand spacing for `Operands::Tokens` (`[operands]`). A space between two
   tokens is only removed when `Dialect::joinable` confirms they still read
   back as the same two tokens, so `start` and `%1` are never glued.
   Directive and section arguments are rendered as written (runs of spaces
   shortened), because NASM splits them at whitespace.
3. Label rewrites (`[labels] add_colon`, `split_inline`). One IR line can
   become two rows (`foo:` and the instruction).
4. Comment marker spacing and the placement mode of comment-only lines.

A code line whose trailing whitespace is significant (`\` + spaces) becomes
a `Verbatim` row, since re-rendering it would trim that whitespace.

### 5.2 Layout engine

Each code line becomes a row of up to four **cells**:

```
 [ label ][ mnemonic/keyword (+prefixes) ][ operands ][ comment ]
```

Each column has a **placement policy** taken from config:

- `Fixed(col)`: start at `col`, or at `prev_end + min_gap` if the previous cell
  runs past it.
- `Block`: start at the smallest column that fits every row in the alignment
  group, but never past `max_column` (outliers fall back to `min_gap`).
- `Gap(n)`: start `n` spaces after the previous cell.

An **alignment group** is a run of consecutive rows aligned together. Groups
break on configurable events: blank line, label-only row, comment-only line,
block opener/closer or change of block depth, directive row, or a verbatim
line (FMT-15). Every row in a group uses the same column positions.

`layout.rs` places columns in this order: labels; keywords (an inline label
pushes its keyword right, and `inline_label_align = "block"` aligns those
across the group); operands (`operand_align`); trailing comments
(`[comments] align`); and finally comment-only rows that follow the code's
indent, which take the column of the next code row (or the previous one at
the end of the file).

The layout engine works on cells and widths and doesn't know anything about
assembly syntax. That's what lets it work for any dialect.

Widths are counted in Unicode scalar values. With `indent.style = "tabs"`,
leading indentation uses tabs where it can, and alignment inside a line uses
spaces (ADR-0015).

### 5.3 Emitter

`emit.rs` joins cells using the computed columns, applies the blank-line
limits (`[blank_lines]`) and drops blank lines at the end of the file. The
caller adds line endings and the BOM. Cells never overlap: if a cell would
start before the previous one ends, it starts one space after it.

### 5.4 Safety check (ADR-0011)

`format::format` ends by comparing `Dialect::check_lines` of the input and
the output (`verify.rs`), allowing only the changes in COR-3. Empty lines
are skipped, and a line that is only `label:` is joined to the next one, so
label splitting passes. A mismatch is a reasm bug: `Formatted::check_failure`
is set, the text is returned unchanged, and the driver reports the first
differing token, leaves the file alone and exits 3.

## 6. Driver and I/O

- Paths go through `driver::discover`. A file is formatted as given. A
  directory is walked recursively for the configured `extensions`, skipping
  hidden directories and `target/`.
- Config is resolved per directory and cached (CFG-1).
- Writes are atomic: write `file.reasm.tmp`, then rename over the original.
  Permissions are kept.
- Unchanged files are never rewritten, so their mtime stays the same.
- Files are formatted one after another in 1.0. NFR-2 is met without threads.
  Parallelism can come later with `std::thread::scope`.

## 7. Errors and diagnostics

- `Diagnostic { severity: Error | Warning | Note, file, line, col, message, code }`
- Codes are stable strings, so they can be silenced: `--allow W001`, or
  `[diagnostics] allow = ["W001"]`. Current codes:
  | Code | Severity | Meaning |
  |---|---|---|
  | W001 | warning | line couldn't be parsed; left as is |
  | W002 | warning | unterminated string; line left as is |
  | W003 | warning | unbalanced block (`%endif` without `%if`, unclosed `%macro`, ...) |
  | N001 | note | unknown word at statement start, treated as a macro call |
  | N002 | note | `reasm: off` without `reasm: on` |
- Rendered as `path:line:col: warning[W001]: message` on stderr (NFR-6).
- Error types are hand-written enums, with no `anyhow`/`thiserror` (ADR-0012).

## 8. Performance plan

- Single pass to read lines. A pre-pass collects `%macro`/`%imacro` names
  defined in the file (ADR-0019).
- Keyword lookup: the word is lowercased once into a stack buffer and looked
  up in one hash map (built on first use from all tables, with a small
  hand-written FxHash-style hasher) that returns a bit per table. This was
  about 6× faster than 14 binary searches (M3 measurement). The sorted
  `&[&str]` tables remain the source of truth and are still tested.
- The IR borrows `&str` slices of the source (`Document<'a>`), so parsing
  doesn't allocate per token. Rows own their rendered cell text.
- The safety check lexes into one flat token buffer per text, with line start
  offsets, and reuses one lexer buffer, so it doesn't allocate per line.
- The emitter writes straight into the output `String`.
- Measured in M3 (2026-11-23, release build, whole process including I/O):
  100k lines of typical code (2.5 MB) in about 94 ms. NASM's test files
  repeated to 100k lines (6.9 MB, long data lines) take about 136 ms. Roughly
  a third of the time is the safety check.
