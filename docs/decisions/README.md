# Architecture Decision Records

Each decision gets its own file: Context → Decision → Alternatives →
Consequences. To change a decision, add a new ADR that supersedes the old one,
and update the old one's status. Don't rewrite history.

Status values: **Accepted**, **Accepted (provisional)** (waiting on an open
question), **Superseded by ADR-NNNN**.

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-rust-single-binary.md) | Rust, single small binary, size-tuned release profile | Accepted |
| [0002](0002-single-package-lib-bin.md) | One package (lib + bin), crate-shaped modules, split later | Accepted |
| [0003](0003-line-oriented-ir.md) | Line-oriented IR, no expression AST | Accepted |
| [0004](0004-dialect-trait.md) | `Dialect` trait, static registry, NASM only for 1.0 | Accepted |
| [0005](0005-config-toml-and-cli.md) | TOML config (nearest file wins) + `--set`; unknown keys are errors | Accepted |
| [0006](0006-verbatim-fallback.md) | Three-level fallback: full → raw operands → verbatim + warning | Accepted |
| [0007](0007-block-bodies.md) | Format block bodies, per-kind nesting indent, `%if 0` verbatim | Accepted |
| [0008](0008-case-only-known-tokens.md) | Case changes only for tokens in keyword tables | Accepted |
| [0009](0009-generated-mnemonic-table.md) | Mnemonic table generated from NASM insns.dat, committed | Accepted |
| [0010](0010-layout-engine.md) | Cell layout with fixed/block/none policies; no `block` defaults, to avoid noisy diffs | Accepted |
| [0011](0011-token-equivalence-check.md) | Token-stream equivalence check before every write | Accepted |
| [0012](0012-minimal-dependencies.md) | Dependencies: serde, toml, clap, similar | Accepted |
| [0013](0013-cli-modes.md) | Paths formatted in place; no paths = stdin → stdout | Accepted |
| [0014](0014-label-columns.md) | Separate local/non-local label columns, both default 0 | Accepted |
| [0015](0015-text-encoding-width.md) | UTF-8 only, width = char count, tabs only for leading indent | Accepted |
| [0016](0016-line-endings.md) | Preserve line endings by default | Accepted |
| [0017](0017-ignore-directives.md) | `reasm: off/on/skip` comments | Accepted |
| [0018](0018-mit-license.md) | MIT license | Accepted |
| [0019](0019-label-detection.md) | Colonless label detection; prefer macro call when ambiguous | Accepted |
| [0020](0020-nasmfmt-default-style.md) | Default style = nasmfmt, except local labels, sections and `%` lines at column 0 | Accepted |
| [0021](0021-macro-arguments.md) | Macro arguments: respace only where NASM drops the spacing; never change case | Accepted |
| [0022](0022-include-pre-pass.md) | Pre-pass follows `%include`; computed macro names freeze keyword case | Accepted |
