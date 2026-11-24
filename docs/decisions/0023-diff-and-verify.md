# ADR-0023: `--diff` with a built-in diff; `--verify` by assembling both texts

**Status:** Accepted (M5)

## Context
CLI-6 asks for a unified diff and CLI-7 for a runtime check that formatting
didn't change what nasm produces (COR-1). The safety check (ADR-0011)
compares token streams with reasm's own lexer, so a lexer bug can hide
from it, as the pasted-name bug did in M4. Only nasm can say whether the
output is really the same.

## Decision

**`--diff`** writes nothing and prints a unified diff of each file that
would change to stdout, then exits 1, like `--check`. The diff is computed
in `src/diff.rs`: it matches lines that occur once on each side
(patience diff), then fills the gaps between them with an LCS table. A
gap too large for the table (over 4M cells) is shown as one block
replacement, which is still a correct diff. Headers are
`--- path\t(original)` / `+++ path\t(formatted)`, so `patch -p0` applies
the output, and a last line without a newline gets
`\ No newline at end of file`.

**`--verify[=FORMAT]`** works with every mode. For each file that
formatting would change, `src/assemble.rs`:

1. writes the original and the formatted text, under the file's own name,
   into two fresh temporary directories, so both outputs embed the same
   source name;
2. runs `nasm -f FORMAT -I<file's dir>/ -I<cwd>/ -I<each nasm.include_paths>/`
   in each directory. Those are the places nasm would have searched;
3. compares the output bytes.

Without `=FORMAT`, `elf64` is tried first and then `bin`, and the first
format the original assembles with is used for both texts. `=` is
required, so `--verify src` treats `src` as a path.

| Result | Effect |
|---|---|
| identical | the formatted text is used |
| the original doesn't assemble on its own | warning W004, and the formatted text is used (the safety check still applies) |
| different bytes, or only the original assembles | internal error, exit 3, and the file is left unchanged |
| nasm can't be run | user error, exit 2; checked once at startup |

A mismatch exits 3, not 2 as CLI.md first said, because it means a reasm
bug, the same as a safety-check failure.

The golden tests use the same module (`tests/assemble.rs`).

## Alternatives
- **A diff crate (`similar`, `diffy`).** These give better diffs on
  pathological input, but they add a dependency and binary size
  (ADR-0012). Formatter diffs mostly pair up line by line, which
  patience+LCS handles well.
- **Shelling out to `diff`.** It isn't available on Windows by default.
- **`--verify` with the project's real build flags.** reasm doesn't know
  the defines or the output format a build uses. Files that need them get
  W004 instead of a false failure. Passing extra nasm arguments can be
  added later if it's needed.

## Consequences
- `--verify` costs two nasm runs per changed file (about 5–20 ms each).
- A file that uses `__?DATE?__`/`__?TIME?__` can differ if the two runs
  fall in different seconds. This is rare, and the file is left unchanged,
  so nothing is lost.
- Verification only sees one file at a time. An included file is
  verified when it's assembled on its own, if it can be.
- Measured on x264 (2026-11-23): without the build's defines, every file
  gets W004 (`x86inc.asm` needs `-DARCH_X86_64`), and the run takes about
  4 minutes because nasm fails late on these macro-heavy files and both
  formats are tried. `--verify` is most useful on files that assemble on
  their own. A way to pass nasm arguments (defines, `-P`) is the natural
  next step if projects like x264 want it.
