//! NASM keyword tables. Most are generated from the NASM sources
//! (`generated.rs`, ADR-0009); the few hand-written ones below group
//! generated entries by how reasm lays them out.
//!
//! All tables are lowercase and sorted by bytes; lookups are ASCII
//! case-insensitive binary searches, because NASM keywords are
//! case-insensitive (verified, NASM.md §3).

#[rustfmt::skip]
mod generated;

use std::cmp::Ordering;

pub use generated::*;

/// Whether `word` is in `table`, ignoring ASCII case.
pub fn contains(table: &[&str], word: &str) -> bool {
    table
        .binary_search_by(|probe| cmp_lower(probe, word))
        .is_ok()
}

fn cmp_lower(lower: &str, word: &str) -> Ordering {
    lower
        .bytes()
        .cmp(word.bytes().map(|b| b.to_ascii_lowercase()))
}

/// Segment registers, which NASM also accepts as prefixes (`es lodsb`).
pub static SEGMENT_REGISTERS: &[&str] = &["cs", "ds", "es", "fs", "gs", "ss"];

/// Standard macros laid out like data keywords.
pub static ALIGN_MACROS: &[&str] = &["align", "alignb"];

/// Standard macros for structures (`struc_body_indent`).
pub static STRUC_OPENERS: &[&str] = &["istruc", "struc"];
pub static STRUC_CLOSERS: &[&str] = &["endstruc", "iend"];
pub static STRUC_OTHER: &[&str] = &["at"];

/// Directives that `directives.section_indent` applies to.
pub static SECTION_DIRECTIVES: &[&str] = &["section", "segment"];

#[cfg(test)]
mod tests {
    use super::*;

    fn all_tables() -> Vec<(&'static str, &'static [&'static str])> {
        vec![
            ("MNEMONICS", MNEMONICS),
            ("DATA_PSEUDO_OPS", DATA_PSEUDO_OPS),
            ("REGISTERS", REGISTERS),
            ("PREFIXES", PREFIXES),
            ("SIZE_KEYWORDS", SIZE_KEYWORDS),
            ("SPECIAL_KEYWORDS", SPECIAL_KEYWORDS),
            ("DIRECTIVES", DIRECTIVES),
            ("PREPROCESSOR_DIRECTIVES", PREPROCESSOR_DIRECTIVES),
            ("STANDARD_MACROS", STANDARD_MACROS),
            ("SEGMENT_REGISTERS", SEGMENT_REGISTERS),
            ("ALIGN_MACROS", ALIGN_MACROS),
            ("STRUC_OPENERS", STRUC_OPENERS),
            ("STRUC_CLOSERS", STRUC_CLOSERS),
            ("STRUC_OTHER", STRUC_OTHER),
            ("SECTION_DIRECTIVES", SECTION_DIRECTIVES),
        ]
    }

    #[test]
    fn tables_are_sorted_lowercase_and_unique() {
        for (name, table) in all_tables() {
            for w in table.windows(2) {
                assert!(w[0] < w[1], "{name}: {:?} !< {:?}", w[0], w[1]);
            }
            for w in table {
                assert_eq!(*w, w.to_ascii_lowercase(), "{name}: {w}");
            }
        }
    }

    #[test]
    fn hand_tables_are_subsets_of_generated_ones() {
        for t in [
            ALIGN_MACROS,
            STRUC_OPENERS,
            STRUC_CLOSERS,
            STRUC_OTHER,
            SECTION_DIRECTIVES,
        ] {
            for w in t {
                assert!(contains(STANDARD_MACROS, w), "{w} is not a standard macro");
            }
        }
        for w in SEGMENT_REGISTERS {
            assert!(contains(REGISTERS, w));
        }
    }

    #[test]
    fn lookups_ignore_case() {
        assert!(contains(MNEMONICS, "mov"));
        assert!(contains(MNEMONICS, "MOV"));
        assert!(contains(MNEMONICS, "CMOVz"));
        assert!(contains(MNEMONICS, "vaddps"));
        assert!(!contains(MNEMONICS, "db"));
        assert!(contains(DATA_PSEUDO_OPS, "equ"));
        assert!(contains(REGISTERS, "ZMM31"));
        assert!(contains(REGISTERS, "r15b"));
        assert!(contains(PREPROCESSOR_DIRECTIVES, "%ifndef"));
        assert!(contains(PREPROCESSOR_DIRECTIVES, "%IMACRO"));
        assert!(contains(PREPROCESSOR_DIRECTIVES, "%elifnidni"));
        assert!(contains(STANDARD_MACROS, "SECTION"));
    }

    /// Verified against NASM 2.16.01 (NASM.md §5): these are symbols.
    #[test]
    fn non_registers() {
        for w in ["rip", "eip", "r8l", "ptr"] {
            assert!(!contains(REGISTERS, w), "{w}");
        }
    }

    #[test]
    fn keyword_tables_do_not_overlap() {
        // Overlaps would make classification ambiguous. `wait` is both a
        // prefix and an instruction in NASM; classify.rs handles it.
        let tables = [
            ("MNEMONICS", MNEMONICS),
            ("DATA_PSEUDO_OPS", DATA_PSEUDO_OPS),
            ("REGISTERS", REGISTERS),
            ("DIRECTIVES", DIRECTIVES),
            ("STANDARD_MACROS", STANDARD_MACROS),
            ("SIZE_KEYWORDS", SIZE_KEYWORDS),
            ("SPECIAL_KEYWORDS", SPECIAL_KEYWORDS),
        ];
        for (i, (an, a)) in tables.iter().enumerate() {
            for (bn, b) in &tables[i + 1..] {
                for w in a.iter() {
                    let allowed = matches!(
                        (*an, *bn, *w),
                        ("DATA_PSEUDO_OPS", "STANDARD_MACROS", "incbin")
                            | ("DIRECTIVES", "STANDARD_MACROS", _)
                    );
                    assert!(allowed || !contains(b, w), "{w} is in both {an} and {bn}");
                }
            }
        }
    }
}
