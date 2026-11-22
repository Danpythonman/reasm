//! The NASM dialect: lexer, keyword tables and line classifier.

use super::Dialect;
use crate::config::Config;
use crate::diag::Diagnostic;
use crate::ir::Document;
use crate::source::Source;

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

    fn parse(&self, source: &Source, config: &Config, diags: &mut Vec<Diagnostic>) -> Document {
        classify::parse(source, config, diags)
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
