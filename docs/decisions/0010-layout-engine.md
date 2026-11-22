# ADR-0010: Dialect-agnostic cell layout with fixed/block/none column policies

**Status:** Accepted

## Decision
Lines are laid out as rows of four cells (label, keyword, operands, comment).
Each column's position comes from a policy: `Fixed(col)`, `Block` (aligned
across an alignment group, capped by `max_column`), or `Gap(n)`. Groups break
on configurable events. See ARCHITECTURE §5.2.

**Defaults (ADR-0020):** operands `none` (single gap, like nasmfmt), comments
`fixed` at column 39. Both avoid `block` alignment, where adding one long
mnemonic re-aligns its neighbors and causes noisy diffs. `block` is still
available for those who prefer it.

## Consequences
The layout engine is the part shared across all dialects, so it gets thorough
unit tests for the column math, independent of any assembly syntax.
