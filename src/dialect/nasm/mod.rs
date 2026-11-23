//! The NASM dialect: lexer, keyword tables and line classifier.

use super::Dialect;
use crate::config::Config;
use crate::diag::Diagnostic;
use crate::ir::{CheckLines, Document};
use crate::source::Source;

pub mod check;
pub mod classify;
pub mod lexer;
pub mod tables;

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

    fn parse<'a>(
        &self,
        source: &Source<'a>,
        config: &Config,
        diags: &mut Vec<Diagnostic>,
    ) -> Document<'a> {
        classify::parse(source, config, diags)
    }

    fn joinable(&self, a: &str, b: &str) -> bool {
        check::joinable(a, b)
    }

    fn check_lines<'a>(&self, text: &'a str) -> CheckLines<'a> {
        check::check_lines(text)
    }

    fn case_insensitive(&self, word: &str) -> bool {
        check::case_insensitive(word)
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
