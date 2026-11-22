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
│   ├── config/              # option structs, TOML loading, --set overrides, defaults, printing
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
│           ├── lexer.rs
│           ├── classify.rs  # line → Line IR, label detection, block structure
│           ├── options.rs   # [nasm] config table
│           └── tables/      # mnemonics (generated), registers, keywords, directives
├── tools/gen-tables/        # generator for the mnemonic table from NASM's insns.dat (ADR-0009)
└── tests/
    ├── cases/               # golden tests (input.asm, expected.asm, reasm.toml)
    └── ...
```

Dependency direction (an arrow means "may use"):

```
cli → driver → { config, dialect, format, diag, source }
dialect → { ir, diag, source, config (own options only) }
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
    Verbatim { text: String, reason: VerbatimReason },  // unparsed / reasm:off / %if 0 / continuation
    Code {
        label:   Option<Label>,
        body:    Option<Body>,
        comment: Option<Comment>,
        standalone_comment_col: Option<u32>,  // original column of a comment-only line
    },
}

struct Label {
    name: String,                   // exact original text
    kind: LabelKind,                // NonLocal | Local | Special(..@) | MacroLocal(%%) | ContextLocal(%$)
    colon: bool,
}

enum Body {
    Instruction { prefixes: Vec<Token>, mnemonic: Token, operands: Operands },
    DataOrEqu   { keyword: Token, operands: Operands },        // db/dw/resb/equ/times/incbin
    Directive   { name: Token, args: Operands, bracketed: bool }, // section/global/bits/[bits 64]
    Preprocessor{ name: Token, rest: RawText, block: BlockRole }, // %define, %macro (Open), %endif (Close), %else (Middle)
    MacroCall   { name: Token, args: RawText },                 // user macro invocation
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
    fn parse(&self, src: &Source, opts: &DialectOptions, diags: &mut Diagnostics) -> Document;
    fn lex_check(&self, text: &str) -> Vec<CheckToken>; // used by the safety check, ADR-0011
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

Applied to each line in the IR:

1. Case transforms for transformable token kinds (`[case]`).
2. Operand spacing for `Operands::Tokens` (`[operands]`). It computes
   `space_before` for each token. A space between two tokens is only removed
   when gluing them together can't lex differently. The dialect lexer decides
   this through `lex_check`, so `%` and `%1`, for example, are never merged.
3. Label rewrites (`[labels] add_colon`, `split_inline`).
4. Blank line limiting (`[blank_lines]`).

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
break on configurable events: blank line, label line, comment-only line,
change of block depth, or a verbatim line (FMT-15). Every row in a group uses
the same column positions.

The layout engine works on cells and widths and doesn't know anything about
assembly syntax. That's what lets it work for any dialect.

Widths are counted in Unicode scalar values. With `indent.style = "tabs"`,
leading indentation uses tabs where it can, and alignment inside a line uses
spaces (ADR-0015).

### 5.3 Emitter

The emitter joins cells using the computed columns, trims trailing whitespace,
applies the line ending, and ensures exactly one final newline.

### 5.4 Safety check (ADR-0011)

Before any write, the driver lexes the original and the formatted text with
the dialect's `lex_check` and compares the two token sequences, allowing only
the changes in COR-3. A mismatch is an internal bug. The file is left alone and
reasm reports the first differing token, then exits 3.

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
- Codes are stable strings (`W001 unparsed-line`, `E101 config-unknown-key`),
  so they can be silenced: `--allow W001`, or `[diagnostics] allow = ["W001"]`.
- Rendered as `path:line:col: warning[W001]: message` on stderr (NFR-6).
- Error types are hand-written enums, with no `anyhow`/`thiserror` (ADR-0012).

## 8. Performance plan

- Single pass to read lines. A pre-pass collects `%macro`/`%imacro` names
  defined in the file (ADR-0019).
- Keyword lookup uses binary search over sorted static `&[&str]` tables with
  ASCII case-insensitive comparison. No hashing crate and no allocation per
  lookup.
- The IR holds `String`s per token for simplicity. If profiling shows
  allocation cost matters, tokens can switch to `&str`/spans into the source.
