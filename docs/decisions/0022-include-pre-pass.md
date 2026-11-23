# ADR-0022: Reading included files in the macro pre-pass

**Status:** Accepted (M4)

## Context
The pre-pass (ADR-0019) collects macro names so reasm can tell labels from
macro calls, keep the case of keywords that a macro shadows (ADR-0008), and
know which macros stringify or take greedy arguments (ADR-0021). It only
read the file being formatted.

The M4 corpus showed what that misses. x264, dav1d and FFmpeg all include
`x86inc.asm`, which redefines instructions as macros whose names are
computed: `AVX_INSTR addps, ...` runs `%macro %1 ...`. Those macros are
case-sensitive, so `case.mnemonic = "upper"` turned `addps` into `ADDPS`,
which skips the macro and assembles the raw instruction. Output differed in
nearly every file of all three projects. The token check (ADR-0011) allows
keyword case changes, so it can't catch this.

## Decision
- `Dialect::parse` receives the file's path. The NASM pre-pass follows
  `%include "name"` (also `'name'` and `<name>`) recursively and collects
  the same information from included files.
- Search order: the including file's directory, the working directory, then
  `nasm.include_paths`. NASM itself searches the working directory and its
  `-I` paths; trying the file's own directory first is a superset that
  works for the usual layouts without configuration.
- At most 64 included files per formatted file. Each file is read once
  (include cycles are cut), and files that can't be found or read are
  skipped silently. Names built by the preprocessor (`%include "%[x]"`) are
  not followed.
- Keyword case is also left unchanged when the file **calls macros whose
  definitions reasm can't see** (note N001). Their definitions are
  somewhere else, and any of them could shadow a keyword. The corpus found
  this in FFmpeg's `huffyuvdsp_template.asm`: it is only included after
  `x86inc.asm`, includes nothing itself, and its `RET` (a case-sensitive
  macro) became `ret`.
- If any visible file defines a macro whose **name is computed**
  (`%macro %1`, `%xdefine %$prefix%2`, `foo%+bar`), any word could be a
  macro, so keyword case is left unchanged for the whole file
  (`Document::keep_keyword_case`). Preprocessor directive names (`%define`)
  can't be macros and still follow `case.preprocessor`. Note N003 says so
  when some `[case]` option asked for a change. Context-local (`%$x`) and
  macro-local (`%%x`) names can't shadow keywords and don't count.

## Alternatives
- **Running the NASM preprocessor:** exact, but needs nasm installed, the
  project's defines and include paths, and would expand macros that the
  formatter must not see expanded.
- **Turning case off for any file with `%include`:** safe, but it would
  disable case options for most real projects even when their includes
  define nothing that matters.

## Consequences
- Formatting a file now depends on the files it includes. Formatting
  stdin without `--stdin-filepath`, or through `format_str`, doesn't read
  includes.
- Case options do nothing in x86inc-based projects, and in any file that
  calls macros from files reasm can't find; `nasm.include_paths` or
  `nasm.known_macros` fix the latter. That is correct: in
  those projects, changing the case of an instruction can change the
  program.
- Projects with includes in unusual places should set
  `nasm.include_paths`; otherwise reasm behaves as before for macros in
  files it can't find.
