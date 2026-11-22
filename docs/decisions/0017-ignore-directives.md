# ADR-0017: In-source ignore directives

**Status:** Accepted

## Decision
Comments that control reasm (matched case-insensitively, with optional spaces
after the marker):
- `; reasm: off` … `; reasm: on`: the lines between are kept byte for byte,
  without even trimming trailing whitespace. The marker lines themselves are
  formatted.
- `; reasm: skip` as a trailing or standalone comment: skip this line (if the
  comment is trailing) or the next line (if it's standalone).

An `off` without a matching `on` runs to the end of the file, with a note.
Ignored regions break alignment groups.

## Consequences
The directive comment uses the dialect's comment marker, so GAS would use
`# reasm: off`. The format is defined once in core and the marker comes from
the dialect.
