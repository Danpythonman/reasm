# ADR-0012: Minimal dependencies

**Status:** Accepted (maintainer chose clap over lexopt, 2026-11-22)

## Decision
Runtime dependencies:

| Crate | Why |
|---|---|
| `serde` (derive) | config structs |
| `toml` (parse only, default features off where possible) | config file parsing and `--print-config` output |
| `clap` (derive; features trimmed to `std`, `derive`, `help`, `usage`, `error-context`) | argument parsing, help and error messages. Shell completions can come later via `clap_complete` |
| `similar` (optional, `diff` feature, default on) | unified diff for `--diff` |

Everything else uses `std`: directory walking, error types, diagnostics
rendering, terminal colors (plain ANSI, disabled when stderr isn't a TTY or
`NO_COLOR` is set).

Dev-dependencies (no effect on binary size) are allowed freely. Candidates:
`insta`, `tempfile`.

## Alternatives
- **lexopt:** tiny (a few KB), but help and errors would be written by hand.
  The maintainer preferred clap's polish over the size savings, and NFR-1 was
  raised to 2 MB to make room.
- **walkdir / ignore:** only needed for `.gitignore`-aware walking, which is
  post-1.0.
- **anyhow / thiserror:** convenient, but hand-written enums are small here.

## Consequences
clap adds roughly 300–500 KB and some compile time. CLI.md stays the source of
truth, and a test checks that every flag in CLI.md exists in `--help`.
