# ADR-0016: Preserve line endings by default

**Status:** Accepted

## Decision
`line_ending = "preserve"` uses the first line ending found in the file (LF if
there is none) for the whole output, which also normalizes mixed files.
`lf` and `crlf` force one style. The output always ends with exactly one line
ending.

## Consequences
Windows users keep CRLF without having to configure anything.
