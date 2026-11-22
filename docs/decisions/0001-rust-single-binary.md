# ADR-0001: Rust, shipped as a single small binary

**Status:** Accepted (maintainer request)

## Context
The maintainer wants one small, dependency-free executable that is easy to
install and to call from editors.

## Decision
reasm is written in Rust, edition 2024, with `rust-version` set to the stable
release at the time M1 starts (1.98 at the time of writing). The release profile
is optimized for size and speed:

```toml
[profile.release]
opt-level = 3        # revisit "s"/"z" only if NFR-1 (< 2 MB) is missed
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

## Alternatives
- **Go** (what nasmfmt uses): also gives single binaries, but they're larger,
  and the maintainer prefers Rust.
- **C:** small, but string handling would be error-prone, and correctness is
  the top priority.

## Consequences
`panic = "abort"` means panics can't be caught to fall back to the original
text. COR-8 (never panic) has to come from code discipline and fuzzing, not
from `catch_unwind`.
