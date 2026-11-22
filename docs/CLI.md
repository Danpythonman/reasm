# Command line interface

```
reasm [OPTIONS] [PATH]...
```

With no `PATH`, reasm reads stdin and writes the result to stdout. A file path
is formatted in place. A directory is walked recursively for files matching
`extensions` (ADR-0013).

## Options

| Option | Description |
|---|---|
| `--check` | Write nothing. Print each file that would change. Exit 1 if any would. |
| `--diff` | Like `--check`, but print a unified diff (SHOULD, CLI-6). |
| `--stdout` | Print formatted output instead of writing files. With more than one file, each is preceded by a `==> path <==` header. |
| `--stdin-filepath <PATH>` | Treat stdin as if it were this file, for config discovery, dialect choice and diagnostics. |
| `--config <FILE>` | Use this config file and skip discovery. |
| `--no-config` | Ignore config files and use defaults plus `--set`. |
| `--set <KEY=VALUE>` | Override a config key, e.g. `--set comments.column=48`. Repeatable. The value is parsed as a TOML value, so strings may be bare if unambiguous (`--set case.mnemonic=upper`). |
| `--dialect <NAME>` | Force a dialect (`nasm`). |
| `--print-config [default\|current]` | Print the default config, or the effective one for the current directory or `--stdin-filepath`, as TOML. Then exit. |
| `--verify[=FORMAT]` | Also assemble the original and formatted text with `nasm -f FORMAT` (default `elf64`) and compare the outputs byte for byte. Needs `nasm` on PATH. Files that don't assemble on their own get a warning, not a failure (SHOULD, CLI-7). |
| `--allow <CODE>` | Silence a warning code. Repeatable. |
| `--deny-warnings` | Treat warnings as errors. |
| `-q, --quiet` | Print only errors. |
| `-v, --verbose` | Also print notes, e.g. lines treated as macro calls. |
| `-h, --help`, `-V, --version` | |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. With `--check`, everything is already formatted. |
| 1 | `--check`/`--diff`: at least one file would change. |
| 2 | User error: bad arguments, bad config, unreadable file, invalid UTF-8, warnings with `--deny-warnings`, or a `--verify` mismatch. |
| 3 | Internal error: the safety check failed (ADR-0011). Please report it. The affected file was not changed. |

When a run hits several conditions, the highest code wins. Other files are
still processed, so one bad file doesn't stop the run.

## Editor integration

Every editor uses the same pattern:

```sh
reasm --stdin-filepath "$FILE" < "$FILE"
```

Examples for Neovim (conform.nvim), VS Code (custom formatter / Run on Save)
and Helix will be added to the README after 1.0.
