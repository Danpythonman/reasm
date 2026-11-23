# reasm

A fast, configurable source formatter for assembly language, shipped as a single
small binary. The first supported dialect is **NASM** (x86 / x86-64). The design
keeps dialect-specific code separate from the shared formatter so that more
dialects (GAS/AT&T, MASM, ...) can be added later.

> **Status:** early development (milestone M3 of [the roadmap](docs/ROADMAP.md)).
> Formatting works with every option in CONFIGURATION.md except
> `format_macro_args`. A safety check refuses to write a file if anything
> other than whitespace, keyword case or a label colon would change. On
> NASM's own test suite, every file that assembles still assembles to the
> same bytes after formatting.

## Usage

```sh
cargo install --path .
reasm file.asm src/            # format in place
reasm --check src/             # CI: exit 1 if anything would change
reasm --stdin-filepath f.asm < f.asm   # editors
reasm --print-config default > reasm.toml
```

## Why another formatter?

The existing NASM formatter (`nasmfmt`, written in Go) makes fixed style choices.
The one that prompted this project: the version the maintainer used indents
`.local:` labels but leaves `global:` labels in column 0. reasm's goal is that **every formatting
decision can be configured**, with safe, conventional defaults. It must also
never change what the assembler produces.

## Example (default style)

The defaults follow nasmfmt, except that section directives, preprocessor
lines and `.local` labels stay in column 0 (ADR-0020):

```nasm
%include "macros.inc"
        global _start

section .text
_start:
        mov rcx, 10                    ; loop counter
.loop:
        dec rcx
        jnz .loop
        ret
```

## Documentation map

| Document | Purpose |
|---|---|
| [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md) | What reasm must and should do, and what it won't do |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Pipeline, modules, data model, dialect extension points |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Every formatting option: type, default, examples |
| [docs/CLI.md](docs/CLI.md) | Command line interface, modes, exit codes |
| [docs/NASM.md](docs/NASM.md) | NASM syntax facts the parser must respect |
| [docs/TESTING.md](docs/TESTING.md) | Test strategy and correctness guarantees |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Milestones toward 1.0 and beyond |
| [docs/decisions/](docs/decisions/README.md) | Architecture Decision Records (ADRs), one per decision |
| [docs/OPEN_QUESTIONS.md](docs/OPEN_QUESTIONS.md) | Decisions still waiting on the maintainer |

## License

MIT. See [LICENSE](LICENSE).
