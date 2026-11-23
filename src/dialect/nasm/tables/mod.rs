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
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::OnceLock;

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

/// One bit per table, for [`flags`].
pub mod flag {
    pub const MNEMONICS: u16 = 1 << 0;
    pub const DATA_PSEUDO_OPS: u16 = 1 << 1;
    pub const REGISTERS: u16 = 1 << 2;
    pub const PREFIXES: u16 = 1 << 3;
    pub const SIZE_KEYWORDS: u16 = 1 << 4;
    pub const SPECIAL_KEYWORDS: u16 = 1 << 5;
    pub const DIRECTIVES: u16 = 1 << 6;
    pub const PREPROCESSOR_DIRECTIVES: u16 = 1 << 7;
    pub const STANDARD_MACROS: u16 = 1 << 8;
    pub const SEGMENT_REGISTERS: u16 = 1 << 9;
    pub const ALIGN_MACROS: u16 = 1 << 10;
    pub const STRUC_OPENERS: u16 = 1 << 11;
    pub const STRUC_CLOSERS: u16 = 1 << 12;
    pub const STRUC_OTHER: u16 = 1 << 13;
    pub const SECTION_DIRECTIVES: u16 = 1 << 14;
}

/// FxHash-style hasher for short keywords: much faster than SipHash and
/// fine for a fixed, trusted key set (NFR-2).
#[derive(Default)]
struct WordHasher(u64);

impl Hasher for WordHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ u64::from(b)).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type Merged = HashMap<&'static [u8], u16, BuildHasherDefault<WordHasher>>;

/// Every keyword with the tables it is in.
fn merged() -> &'static Merged {
    static MERGED: OnceLock<Merged> = OnceLock::new();
    MERGED.get_or_init(|| {
        let mut map = Merged::default();
        for (table, bit) in [
            (MNEMONICS, flag::MNEMONICS),
            (DATA_PSEUDO_OPS, flag::DATA_PSEUDO_OPS),
            (REGISTERS, flag::REGISTERS),
            (PREFIXES, flag::PREFIXES),
            (SIZE_KEYWORDS, flag::SIZE_KEYWORDS),
            (SPECIAL_KEYWORDS, flag::SPECIAL_KEYWORDS),
            (DIRECTIVES, flag::DIRECTIVES),
            (PREPROCESSOR_DIRECTIVES, flag::PREPROCESSOR_DIRECTIVES),
            (STANDARD_MACROS, flag::STANDARD_MACROS),
            (SEGMENT_REGISTERS, flag::SEGMENT_REGISTERS),
            (ALIGN_MACROS, flag::ALIGN_MACROS),
            (STRUC_OPENERS, flag::STRUC_OPENERS),
            (STRUC_CLOSERS, flag::STRUC_CLOSERS),
            (STRUC_OTHER, flag::STRUC_OTHER),
            (SECTION_DIRECTIVES, flag::SECTION_DIRECTIVES),
        ] {
            for w in table {
                *map.entry(w.as_bytes()).or_default() |= bit;
            }
        }
        map
    })
}

/// The [`flag`]s of every table that contains `word`, ignoring ASCII case.
/// One hash lookup, with the word lowercased once (NFR-2).
pub fn flags(word: &str) -> u16 {
    let bytes = word.as_bytes();
    let mut buf = [0u8; 24];
    if bytes.is_empty() || bytes.len() > buf.len() {
        return 0;
    }
    let lower = &mut buf[..bytes.len()];
    lower.copy_from_slice(bytes);
    lower.make_ascii_lowercase();
    merged().get(&*lower).copied().unwrap_or(0)
}

/// Whether `word` is in any table in `mask`.
pub fn is(word: &str, mask: u16) -> bool {
    flags(word) & mask != 0
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
    fn merged_lookup_matches_tables() {
        for (name, table) in all_tables() {
            for w in table {
                assert!(contains(table, w), "{name}: {w}");
                let bit = all_tables().iter().position(|t| t.0 == name).unwrap();
                assert!(is(&w.to_ascii_uppercase(), 1 << bit), "{name}: {w}");
            }
        }
        assert_eq!(flags("notakeyword"), 0);
        assert_eq!(flags(""), 0);
        assert!(longest() <= 24, "raise the lookup buffer");
    }

    fn longest() -> usize {
        all_tables()
            .iter()
            .flat_map(|t| t.1.iter())
            .map(|w| w.len())
            .max()
            .unwrap()
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
