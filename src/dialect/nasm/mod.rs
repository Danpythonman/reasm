//! The NASM dialect. M1 has only the pieces the skeleton formatter needs;
//! the lexer, classifier and keyword tables arrive in M2 (ROADMAP).

use super::Dialect;

pub struct Nasm;

impl Dialect for Nasm {
    fn name(&self) -> &'static str {
        "nasm"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["asm", "nasm", "inc"]
    }

    fn trailing_whitespace_significant(&self, line: &str) -> bool {
        let trimmed = line.trim_end();
        trimmed.len() != line.len() && trimmed.ends_with('\\')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backslash_followed_by_spaces_is_significant() {
        assert!(Nasm.trailing_whitespace_significant("%define X 1 + \\   "));
        assert!(!Nasm.trailing_whitespace_significant("%define X 1 + \\"));
        assert!(!Nasm.trailing_whitespace_significant("mov eax, 1   "));
    }
}
