# ADR-0005: TOML config file + `--set` overrides; unknown keys are errors

**Status:** Accepted (maintainer choice)

## Decision
- Files: `reasm.toml` or `.reasm.toml`. Discovery walks up from the formatted
  file's directory, and the **nearest file wins**, with no merging of parent
  files. If both names are in one directory, that's an error.
- `--config FILE` skips discovery. `--no-config` uses defaults only.
- `--set dotted.key=value` overrides individual keys. One generic flag covers
  every option, so there's no per-option CLI flag to keep in sync.
- Precedence: built-in default < dialect default < file < `--set`.
- Parsing uses `serde` + `toml` with `deny_unknown_fields`, so typos are
  reported with the key and line.
- Every option is documented in CONFIGURATION.md, and a test checks that each
  key in the default struct appears there.

## Alternatives
- **Merging parent configs (like EditorConfig):** more flexible, but makes it
  harder to tell where a setting came from. It can be added later with
  `inherit = true`.
- **Dedicated CLI flags per option:** discoverable through `--help`, but
  doubles the maintenance and makes the binary bigger.

## Consequences
Adds `serde` and `toml` as dependencies (ADR-0012). There's no `version` key in
1.0. If defaults ever change incompatibly, a `style_edition` key (like
rustfmt's) can be introduced.
