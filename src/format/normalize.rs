//! IR → rows of rendered cells (ARCHITECTURE §5.1): case transforms,
//! operand spacing, colons, label splitting and each cell's base column.
//! Nothing here knows about the final columns; that's `layout`.

use crate::config::*;
use crate::dialect::Dialect;
use crate::ir::{self, *};

/// One output line before layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row<'a> {
    Blank,
    /// Written as is. `keep` rows are never dropped as trailing blanks
    /// (they're inside `reasm: off`).
    Verbatim {
        text: &'a str,
        keep: bool,
    },
    /// A comment-only line.
    Comment {
        text: String,
        place: CommentPlace,
        depth: BlockDepth,
    },
    Code(CodeRow),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentPlace {
    Column(u32),
    /// The indent of the next code row (or the previous one at the end).
    FollowCode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeRow {
    pub label: Option<LabelCell>,
    pub keyword: Option<KeywordCell>,
    pub operands: Option<String>,
    /// Rendered with its marker.
    pub comment: Option<String>,
    pub depth: BlockDepth,
    pub role: BlockRole,
    /// Section, directive or preprocessor line (`alignment.break_on`).
    pub directive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelCell {
    pub text: String,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordCell {
    /// Prefixes and keyword (or a whole `[...]` directive).
    pub text: String,
    /// Column when no inline label pushes it right.
    pub column: u32,
    pub gap: OperandGap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandGap {
    /// Placed by `[instructions] operand_align`.
    Instruction,
    /// A fixed number of spaces after the keyword.
    Spaces(u32),
}

pub fn rows<'a>(
    doc: &Document,
    lines: &[&'a str],
    cfg: &Config,
    dialect: &dyn Dialect,
) -> Vec<Row<'a>> {
    let n = Normalizer { cfg, dialect };
    let mut out = Vec::with_capacity(lines.len() + lines.len() / 8);
    for (line, &raw) in doc.lines.iter().zip(lines) {
        match &line.content {
            LineContent::Blank => out.push(Row::Blank),
            LineContent::Verbatim(reason) => {
                let keep = *reason == VerbatimReason::Disabled;
                let text = if keep || dialect.trailing_whitespace_significant(raw) {
                    raw
                } else {
                    raw.trim_end()
                };
                out.push(Row::Verbatim { text, keep });
            }
            // `\` + spaces at the end is meaningful (NASM.md §1), and
            // re-rendering the line would trim it.
            LineContent::Code(_) if dialect.trailing_whitespace_significant(raw) => {
                out.push(Row::Verbatim {
                    text: raw,
                    keep: false,
                });
            }
            LineContent::Code(code) => n.code(line, code, &mut out),
        }
    }
    out
}

struct Normalizer<'c> {
    cfg: &'c Config,
    dialect: &'c dyn Dialect,
}

fn space_rule(mode: OperatorSpacing, original: bool) -> bool {
    match mode {
        OperatorSpacing::Preserve => original,
        OperatorSpacing::Spaced => true,
        OperatorSpacing::Compact => false,
    }
}

/// Whether a `+`/`-` after `prev` is unary (NASM.md §6).
fn unary_after(prev: Option<&Token>) -> bool {
    prev.is_none_or(|p| {
        matches!(
            p.kind,
            TokenKind::OpenBracket
                | TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::OpenParen
                | TokenKind::Comma
                | TokenKind::Colon
                | TokenKind::Operator
        )
    })
}

impl Normalizer<'_> {
    /// Appends `tok` to `out` with its `[case]` transform.
    fn push_case(&self, out: &mut String, tok: &Token) {
        let c = &self.cfg.case;
        let style = match tok.kind {
            TokenKind::Mnemonic | TokenKind::Prefix => c.mnemonic,
            TokenKind::Register => c.register,
            TokenKind::SizeKeyword | TokenKind::OperandKeyword => c.size_keyword,
            TokenKind::DataKeyword => c.data,
            TokenKind::Directive => c.directive,
            TokenKind::PreprocessorName => c.preprocessor,
            _ => CaseStyle::Preserve,
        };
        let start = out.len();
        out.push_str(tok.text);
        match style {
            CaseStyle::Preserve => {}
            CaseStyle::Lower => out[start..].make_ascii_lowercase(),
            CaseStyle::Upper => out[start..].make_ascii_uppercase(),
        }
    }

    fn block_indent(&self, d: BlockDepth) -> u32 {
        let p = &self.cfg.preprocessor;
        u32::from(d.macro_def) * p.macro_body_indent
            + u32::from(d.conditional) * p.conditional_body_indent
            + u32::from(d.rep) * p.rep_body_indent
            + u32::from(d.struc) * p.struc_body_indent
    }

    fn comment(&self, c: &Comment) -> String {
        let banner = c.text.starts_with([
            ';', '-', '=', '*', '#', '~', '+', '/', '_', '!', '|', '^', '<', '>',
        ]);
        let space = self.cfg.comments.space_after_marker == MarkerSpace::Ensure
            && !c.text.is_empty()
            && !c.text.starts_with(char::is_whitespace)
            && !banner;
        format!("{}{}{}", c.marker, if space { " " } else { "" }, c.text)
    }

    fn code(&self, line: &Line, code: &CodeLine, out: &mut Vec<Row<'_>>) {
        let depth = line.depth;
        let indent = self.block_indent(depth);
        let comment = code.comment.as_ref().map(|c| self.comment(c));

        if code.label.is_none() && code.body.is_none() {
            let cfg = &self.cfg.comments;
            let place = match cfg.standalone {
                StandaloneComments::Preserve => CommentPlace::Column(code.original_column),
                StandaloneComments::Zero => CommentPlace::Column(0),
                StandaloneComments::Code => CommentPlace::FollowCode,
                StandaloneComments::Smart if code.original_column == 0 => CommentPlace::Column(0),
                StandaloneComments::Smart => CommentPlace::FollowCode,
            };
            out.push(Row::Comment {
                text: comment.unwrap_or_default(),
                place,
                depth,
            });
            return;
        }

        let labels = &self.cfg.labels;
        let kind = code.body.as_ref().map(|b| b.kind);
        // A word without a colon before an instruction or macro call may
        // itself be a macro call (`alignmode nop, 32`), so it is never
        // split off or given a colon: either would change the program if
        // the guess is wrong (ADR-0019). Before data (`msg db 1`) it is
        // always a label.
        let guessed = code.label.as_ref().is_some_and(|l| !l.colon)
            && matches!(kind, Some(BodyKind::Instruction | BodyKind::MacroCall));
        let split = code.label.is_some()
            && !guessed
            && match (labels.split_inline, kind) {
                (_, None) | (SplitInline::Preserve, _) => false,
                (_, Some(BodyKind::Instruction | BodyKind::MacroCall)) => true,
                (SplitInline::All, Some(BodyKind::Data)) => true,
                _ => false,
            };
        let label = code.label.as_ref().map(|l| {
            let colon = l.colon
                || split
                || (kind.is_some()
                    && !guessed
                    && labels.add_colon
                    && (kind != Some(BodyKind::Equ) || labels.add_colon_to_equ));
            // A lone label without a colon stays in column 0: indented, it
            // would read back as a macro call (ADR-0019).
            let column = if kind.is_none() && !l.colon {
                0
            } else if l.kind.is_local() {
                indent + labels.local_indent
            } else {
                indent + labels.indent
            };
            LabelCell {
                text: format!("{}{}", l.name, if colon { ":" } else { "" }),
                column,
            }
        });

        let Some(body) = &code.body else {
            out.push(Row::Code(CodeRow {
                label,
                keyword: None,
                operands: None,
                comment,
                depth,
                role: BlockRole::None,
                directive: false,
            }));
            return;
        };

        let (keyword, operands) = self.body(body, indent);
        let row = CodeRow {
            label: None,
            keyword: Some(keyword),
            operands,
            comment,
            depth,
            role: body.role,
            directive: matches!(
                body.kind,
                BodyKind::Section | BodyKind::Directive | BodyKind::Preprocessor
            ),
        };
        if split {
            out.push(Row::Code(CodeRow {
                label,
                keyword: None,
                operands: None,
                comment: None,
                depth,
                role: BlockRole::None,
                directive: false,
            }));
            out.push(Row::Code(row));
        } else {
            out.push(Row::Code(CodeRow { label, ..row }));
        }
    }

    fn body(&self, body: &Body, indent: u32) -> (KeywordCell, Option<String>) {
        let cfg = self.cfg;
        let instruction = indent + cfg.indent.instruction;
        let directive_col = |d: DirectiveIndent| match d {
            DirectiveIndent::Zero => indent,
            DirectiveIndent::Instruction => instruction,
        };
        let (column, gap) = match body.kind {
            BodyKind::Section => (
                directive_col(cfg.directives.section_indent),
                OperandGap::Spaces(cfg.directives.arg_gap),
            ),
            BodyKind::Directive => (
                directive_col(cfg.directives.indent),
                OperandGap::Spaces(cfg.directives.arg_gap),
            ),
            BodyKind::Preprocessor => (
                match cfg.preprocessor.indent {
                    PreprocessorIndent::Zero => 0,
                    PreprocessorIndent::Nested => indent,
                    PreprocessorIndent::Instruction => instruction,
                },
                OperandGap::Spaces(cfg.preprocessor.arg_gap),
            ),
            _ => (instruction, OperandGap::Instruction),
        };

        let mut text = String::new();
        let mut prev: Option<&Token> = None;
        for p in &body.prefixes {
            if let Some(prev) = prev
                && (p.space_before || !self.dialect.joinable(prev.text, p.text))
            {
                text.push(' ');
            }
            self.push_case(&mut text, p);
            prev = Some(p);
        }
        if !text.is_empty() {
            text.push(' ');
        }
        self.push_case(&mut text, &body.keyword);

        let operands = match &body.operands {
            ir::Operands::Raw("") => None,
            ir::Operands::Raw(raw) => Some(raw.to_string()),
            ir::Operands::Tokens(t) if t.is_empty() => None,
            // Directive arguments are whitespace-separated words to NASM
            // (`section __LD,__compact_unwind`), so only runs of spaces
            // are shortened.
            ir::Operands::Tokens(t)
                if matches!(body.kind, BodyKind::Section | BodyKind::Directive) =>
            {
                Some(self.as_written(t))
            }
            ir::Operands::Tokens(t) => Some(self.operands(t)),
        };

        if body.bracketed {
            let spaces = match gap {
                OperandGap::Spaces(n) => n,
                OperandGap::Instruction => 1,
            };
            let mut whole = format!("[{text}");
            if let Some(ops) = operands {
                whole.push_str(&" ".repeat(spaces as usize));
                whole.push_str(&ops);
            }
            whole.push(']');
            let cell = KeywordCell {
                text: whole,
                column,
                gap,
            };
            return (cell, None);
        }
        (KeywordCell { text, column, gap }, operands)
    }

    /// Tokens with a single space wherever the source had whitespace.
    fn as_written(&self, tokens: &[Token]) -> String {
        let mut out = String::new();
        for (i, t) in tokens.iter().enumerate() {
            if i > 0 && t.space_before {
                out.push(' ');
            }
            self.push_case(&mut out, t);
        }
        out
    }

    /// Renders an operand list with the `[operands]` spacing rules
    /// (CONFIGURATION.md). A space is only ever removed when the dialect
    /// confirms the two tokens still read back the same.
    fn operands(&self, tokens: &[Token]) -> String {
        let o = &self.cfg.operands;
        let mut out = String::new();
        let mut bracket_depth = 0u32;
        // Whether the previous token was a unary `+`/`-`.
        let mut prev_unary = false;
        for (i, t) in tokens.iter().enumerate() {
            let prev = i.checked_sub(1).map(|p| &tokens[p]);
            let in_memory = bracket_depth > 0;
            let unary = matches!(t.kind, TokenKind::Plus | TokenKind::Minus) && unary_after(prev);
            if let Some(p) = prev {
                let orig = t.space_before;
                let padding = |orig| match o.bracket_padding {
                    BracketPadding::Preserve => orig,
                    BracketPadding::None => false,
                    BracketPadding::Padded => true,
                };
                let segment = o.segment_colon == SegmentColon::Compact;
                let sign = |k| matches!(k, TokenKind::Plus | TokenKind::Minus);
                let mut space = if t.kind == TokenKind::Comma {
                    o.space_before_comma
                } else if p.kind == TokenKind::Comma {
                    o.space_after_comma
                } else if p.kind == TokenKind::OpenBracket || t.kind == TokenKind::CloseBracket {
                    padding(orig)
                } else if matches!(p.kind, TokenKind::SizeKeyword | TokenKind::OperandKeyword)
                    && o.size_keyword_space
                {
                    true
                } else if segment
                    && ((t.kind == TokenKind::Colon && p.kind == TokenKind::Register)
                        || (p.kind == TokenKind::Colon
                            && i >= 2
                            && tokens[i - 2].kind == TokenKind::Register))
                {
                    false
                } else if in_memory && sign(p.kind) && prev_unary {
                    // Never a space after a unary sign, except as written.
                    o.memory_plus_minus == OperatorSpacing::Preserve && orig
                } else if in_memory && ((sign(t.kind) && !unary) || sign(p.kind)) {
                    space_rule(o.memory_plus_minus, orig)
                } else if in_memory && (t.kind == TokenKind::Star || p.kind == TokenKind::Star) {
                    space_rule(o.memory_scale, orig)
                } else {
                    orig
                };
                if !space && !self.dialect.joinable(p.text, t.text) {
                    space = true;
                }
                if space {
                    out.push(' ');
                }
            }
            self.push_case(&mut out, t);
            match t.kind {
                TokenKind::OpenBracket => bracket_depth += 1,
                TokenKind::CloseBracket => bracket_depth = bracket_depth.saturating_sub(1),
                _ => {}
            }
            prev_unary = unary;
        }
        out
    }
}
