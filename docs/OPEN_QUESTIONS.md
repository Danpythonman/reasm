# Open questions

These need the maintainer's input. Each answer is recorded here and in the
affected ADR or doc.

## Answered

| Question | Answer | Recorded in |
|---|---|---|
| How are options specified? | TOML file + CLI `--set` overrides | ADR-0005 |
| License? | MIT | ADR-0018 |
| Unparseable lines? | Verbatim + warning | ADR-0006 |
| Format macro/conditional bodies? | Yes, with nesting indent options | ADR-0007 |
| Dialects? | NASM only for now, modular for more later | ADR-0004 |
| Q1: Default style | nasmfmt's style, but `.local` labels and `section` lines are not indented | ADR-0020, CONFIGURATION.md |
| Q2: Argument parser | clap (binary budget raised to 2 MB) | ADR-0012, NFR-1 |
| Q3: Writing behavior | In place | ADR-0013 |
| Q5: Copyright holder | Daniel Di Giovanni | LICENSE, ADR-0018 |
| Q4: `.inc` files | Included in default `extensions` | CONFIGURATION.md |
| Q7: Preprocessor indent | Column 0 | ADR-0020, CONFIGURATION.md |
| Q8: `global`/`extern` indent | Stay indented (nasmfmt) | ADR-0020 |
| Q6: Publishing | crates.io and GitHub, deferred until 1.0. The name `reasm` was free on crates.io as of 2026-11-22. | ROADMAP 1.0 |

## Open

None. Planning (M0) is complete. Next is M1 (ROADMAP.md).
