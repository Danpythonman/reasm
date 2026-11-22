//! Turns NASM source lines into the IR: line classification (NASM.md §4,
//! ADR-0019), block structure (ADR-0007), `%if 0` regions, and
//! `reasm: off/on/skip` (ADR-0017).

use std::collections::HashSet;

use super::lexer::{self, LexError, Tok};
use super::tables::{self, contains};
use crate::config::{Config, NasmOptions};
use crate::diag::{Diagnostic, Severity};
use crate::ir::*;
use crate::source::Source;

/// Diagnostic codes (ARCHITECTURE §7).
pub mod codes {
    pub const UNPARSED: &str = "W001";
    pub const UNTERMINATED_STRING: &str = "W002";
    pub const UNBALANCED_BLOCK: &str = "W003";
    pub const ASSUMED_MACRO: &str = "N001";
    pub const UNCLOSED_OFF: &str = "N002";
}

pub fn parse(source: &Source, config: &Config, diags: &mut Vec<Diagnostic>) -> Document {
    let macros = Macros::collect(&source.lines, &config.nasm);
    let classifier = Classifier {
        macros,
        tab_width: config.indent.tab_width.max(1),
    };
    let mut b = Builder::default();

    for (idx, &raw) in source.lines.iter().enumerate() {
        let number = idx as u32 + 1;
        let continues = raw.ends_with('\\');
        let in_continuation = b.prev_continues || continues;
        b.prev_continues = continues;
        let marker = Marker::of(raw);

        if b.disabled_since.is_some() {
            if marker != Some(Marker::On) {
                b.push(number, LineContent::Verbatim(VerbatimReason::Disabled));
                continue;
            }
            b.disabled_since = None;
        }
        if b.skip_next {
            b.skip_next = false;
            b.push(number, LineContent::Verbatim(VerbatimReason::Disabled));
            continue;
        }
        if let Some(nest) = b.commented_out {
            match commented_out_step(raw, nest) {
                CommentedStep::Inside(n) => {
                    b.commented_out = Some(n);
                    b.push(number, LineContent::Verbatim(VerbatimReason::CommentedOut));
                    continue;
                }
                CommentedStep::Leave => b.commented_out = None,
            }
        }
        if in_continuation {
            b.push(number, LineContent::Verbatim(VerbatimReason::Continuation));
            continue;
        }

        let (mut content, role) = classifier.classify(raw, number, diags);
        let mut skip_this = false;
        if let LineContent::Code(code) = &content {
            let standalone = code.label.is_none() && code.body.is_none();
            match marker {
                Some(Marker::Off) => b.disabled_since = Some(number),
                Some(Marker::Skip) if standalone => b.skip_next = true,
                Some(Marker::Skip) => skip_this = true,
                _ => {}
            }
            if code.body.as_ref().is_some_and(is_if_zero) {
                b.commented_out = Some(0);
            }
        }
        if skip_this {
            content = LineContent::Verbatim(VerbatimReason::Disabled);
        }
        b.push_with_role(number, content, role, diags);
    }
    b.finish(diags)
}

fn diag(
    severity: Severity,
    code: &'static str,
    line: u32,
    col: u32,
    message: String,
) -> Diagnostic {
    Diagnostic {
        severity,
        code,
        line,
        col,
        message,
    }
}

/// 1-based character column of byte `offset` in `text`.
fn column(text: &str, offset: usize) -> u32 {
    text[..offset.min(text.len())].chars().count() as u32 + 1
}

// ---------------------------------------------------------------------------
// Macro names (pre-pass, ADR-0019)

struct Macros {
    exact: HashSet<String>,
    /// Lowercased names of case-insensitive macros.
    folded: HashSet<String>,
}

impl Macros {
    fn collect(lines: &[&str], opts: &NasmOptions) -> Self {
        let mut m = Self {
            exact: opts.known_macros.iter().cloned().collect(),
            folded: opts
                .known_imacros
                .iter()
                .map(|s| s.to_ascii_lowercase())
                .collect(),
        };
        for line in lines {
            let mut words = line.split_whitespace();
            let (Some(directive), Some(name)) = (words.next(), words.next()) else {
                continue;
            };
            if !directive.starts_with('%') {
                continue;
            }
            let name = name.split('(').next().unwrap_or(name).to_string();
            match directive.to_ascii_lowercase().as_str() {
                "%macro" | "%rmacro" | "%define" | "%xdefine" => m.exact.insert(name),
                "%imacro" | "%irmacro" | "%idefine" | "%ixdefine" => {
                    m.folded.insert(name.to_ascii_lowercase())
                }
                _ => false,
            };
        }
        m
    }

    fn contains(&self, word: &str) -> bool {
        self.exact.contains(word) || self.folded.contains(&word.to_ascii_lowercase())
    }
}

// ---------------------------------------------------------------------------
// reasm: off / on / skip (ADR-0017) and %if 0 (ADR-0007)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    Off,
    On,
    Skip,
}

impl Marker {
    fn of(raw: &str) -> Option<Self> {
        let (_, comment) = lexer::split_comment(raw).ok()?;
        let compact: String = comment?.chars().filter(|c| !c.is_whitespace()).collect();
        match compact.to_ascii_lowercase().as_str() {
            "reasm:off" => Some(Self::Off),
            "reasm:on" => Some(Self::On),
            "reasm:skip" => Some(Self::Skip),
            _ => None,
        }
    }
}

fn is_if_zero(body: &Body) -> bool {
    body.kind == BodyKind::Preprocessor
        && body.keyword.text.eq_ignore_ascii_case("%if")
        && matches!(&body.operands, Operands::Raw(r) if r == "0")
}

enum CommentedStep {
    /// Still inside; the new nesting depth of `%if`s seen inside.
    Inside(u32),
    /// This line ends the region (`%endif`, `%elif`, `%else` at depth 0)
    /// and is classified normally.
    Leave,
}

fn commented_out_step(raw: &str, nest: u32) -> CommentedStep {
    let word = raw
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == ';')
        .next()
        .unwrap_or("");
    let lower = word.to_ascii_lowercase();
    let is_pp = lower.starts_with('%') && contains(tables::PREPROCESSOR_DIRECTIVES, &lower);
    if is_pp && lower.starts_with("%if") {
        CommentedStep::Inside(nest + 1)
    } else if is_pp && lower == "%endif" {
        if nest == 0 {
            CommentedStep::Leave
        } else {
            CommentedStep::Inside(nest - 1)
        }
    } else if is_pp && nest == 0 && (lower.starts_with("%elif") || lower == "%else") {
        CommentedStep::Leave
    } else {
        CommentedStep::Inside(nest)
    }
}

// ---------------------------------------------------------------------------
// Block structure

#[derive(Default)]
struct Builder {
    lines: Vec<Line>,
    depth: BlockDepth,
    /// Open blocks with the line that opened them.
    stack: Vec<(BlockKind, u32)>,
    prev_continues: bool,
    disabled_since: Option<u32>,
    skip_next: bool,
    commented_out: Option<u32>,
}

fn block_name(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Macro => "%macro",
        BlockKind::Conditional => "%if",
        BlockKind::Rep => "%rep",
        BlockKind::Struc => "struc",
    }
}

impl Builder {
    fn push(&mut self, number: u32, content: LineContent) {
        self.lines.push(Line {
            number,
            depth: self.depth,
            content,
        });
    }

    fn close_block(&mut self, kind: BlockKind) {
        let d = self.depth.get_mut(kind);
        *d = d.saturating_sub(1);
    }

    fn push_with_role(
        &mut self,
        number: u32,
        content: LineContent,
        role: BlockRole,
        diags: &mut Vec<Diagnostic>,
    ) {
        let warn = |message: String| {
            diag(
                Severity::Warning,
                codes::UNBALANCED_BLOCK,
                number,
                0,
                message,
            )
        };
        let mut depth = self.depth;
        match role {
            BlockRole::None => {}
            BlockRole::Open(kind) => {
                self.stack.push((kind, number));
                *self.depth.get_mut(kind) += 1;
            }
            BlockRole::Middle(kind) => {
                if self.stack.last().map(|s| s.0) == Some(kind) {
                    let d = depth.get_mut(kind);
                    *d = d.saturating_sub(1);
                } else {
                    diags.push(warn(format!(
                        "this line has no matching `{}`",
                        block_name(kind)
                    )));
                }
            }
            BlockRole::Close(kind) => match self.stack.iter().rposition(|s| s.0 == kind) {
                Some(pos) => {
                    let inner: Vec<_> = self.stack.drain(pos + 1..).collect();
                    for (inner_kind, line) in inner.into_iter().rev() {
                        diags.push(warn(format!(
                            "`{}` opened on line {line} is not closed before this line",
                            block_name(inner_kind)
                        )));
                        self.close_block(inner_kind);
                    }
                    self.stack.pop();
                    self.close_block(kind);
                    depth = self.depth;
                }
                None => diags.push(warn(format!(
                    "this line has no matching `{}`",
                    block_name(kind)
                ))),
            },
        }
        self.lines.push(Line {
            number,
            depth,
            content,
        });
    }

    fn finish(self, diags: &mut Vec<Diagnostic>) -> Document {
        for (kind, line) in &self.stack {
            diags.push(diag(
                Severity::Warning,
                codes::UNBALANCED_BLOCK,
                *line,
                0,
                format!("`{}` is never closed", block_name(*kind)),
            ));
        }
        if let Some(line) = self.disabled_since {
            diags.push(diag(
                Severity::Note,
                codes::UNCLOSED_OFF,
                line,
                0,
                "`reasm: off` has no matching `reasm: on`; the rest of the file is left as is"
                    .into(),
            ));
        }
        Document { lines: self.lines }
    }
}

// ---------------------------------------------------------------------------
// Line classification

/// What a word at the start of a statement is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kw {
    Times,
    Prefix,
    SegmentRegister,
    Mnemonic,
    Data,
    Equ,
    Struc(BlockRole),
    Section,
    /// `macro_based`: a standard macro (`global`, `bits`), which can't be
    /// a label even with a colon. Plain directives (`list`, `map`) can.
    Directive {
        macro_based: bool,
    },
}

fn keyword(word: &str) -> Option<Kw> {
    use tables::*;
    let kw = if word.eq_ignore_ascii_case("times") {
        Kw::Times
    } else if word.eq_ignore_ascii_case("equ") {
        Kw::Equ
    } else if contains(DATA_PSEUDO_OPS, word) || contains(ALIGN_MACROS, word) {
        Kw::Data
    } else if contains(STRUC_OPENERS, word) {
        Kw::Struc(BlockRole::Open(BlockKind::Struc))
    } else if contains(STRUC_CLOSERS, word) {
        Kw::Struc(BlockRole::Close(BlockKind::Struc))
    } else if contains(STRUC_OTHER, word) {
        Kw::Struc(BlockRole::None)
    } else if contains(SECTION_DIRECTIVES, word) {
        Kw::Section
    } else if contains(STANDARD_MACROS, word) {
        Kw::Directive { macro_based: true }
    } else if contains(DIRECTIVES, word) {
        Kw::Directive { macro_based: false }
    } else if contains(MNEMONICS, word) {
        Kw::Mnemonic
    } else if contains(PREFIXES, word) {
        Kw::Prefix
    } else if contains(SEGMENT_REGISTERS, word) {
        Kw::SegmentRegister
    } else {
        return None;
    };
    Some(kw)
}

/// Words that can't be a label even when followed by a colon.
fn reserved(word: &str) -> bool {
    match keyword(word) {
        Some(Kw::Directive { macro_based: false }) => false,
        Some(_) => true,
        None => {
            contains(tables::REGISTERS, word)
                || contains(tables::SIZE_KEYWORDS, word)
                || contains(tables::SPECIAL_KEYWORDS, word)
        }
    }
}

fn label_kind(name: &str) -> LabelKind {
    let name = name.strip_prefix('$').unwrap_or(name);
    if name.starts_with("..") {
        LabelKind::Special
    } else if name.starts_with('.') {
        LabelKind::Local
    } else if name.starts_with("%%") {
        LabelKind::MacroLocal
    } else if name.starts_with("%$") {
        LabelKind::ContextLocal
    } else {
        LabelKind::NonLocal
    }
}

fn pp_role(lower: &str) -> BlockRole {
    use BlockKind::*;
    match lower {
        "%macro" | "%imacro" | "%rmacro" | "%irmacro" => BlockRole::Open(Macro),
        "%endmacro" | "%endm" => BlockRole::Close(Macro),
        "%rep" => BlockRole::Open(Rep),
        "%endrep" => BlockRole::Close(Rep),
        "%endif" => BlockRole::Close(Conditional),
        "%else" => BlockRole::Middle(Conditional),
        _ if lower.starts_with("%elif") => BlockRole::Middle(Conditional),
        _ if lower.starts_with("%if") => BlockRole::Open(Conditional),
        _ => BlockRole::None,
    }
}

/// Kind of an operand token: registers and keywords are recognized by
/// table; `$name` is always a symbol (ADR-0008).
fn operand_kind(tok: &Tok, text: &str) -> TokenKind {
    if tok.kind != TokenKind::Identifier || text.starts_with('$') {
        return tok.kind;
    }
    if contains(tables::REGISTERS, text) {
        TokenKind::Register
    } else if contains(tables::SIZE_KEYWORDS, text) {
        TokenKind::SizeKeyword
    } else if contains(tables::SPECIAL_KEYWORDS, text) {
        TokenKind::OperandKeyword
    } else {
        TokenKind::Identifier
    }
}

fn token(code: &str, tok: &Tok, kind: TokenKind) -> Token {
    Token {
        kind,
        text: code[tok.start..tok.end].to_string(),
        space_before: tok.space_before,
    }
}

fn operand_tokens(code: &str, toks: &[Tok]) -> Operands {
    Operands::Tokens(
        toks.iter()
            .map(|t| token(code, t, operand_kind(t, &code[t.start..t.end])))
            .collect(),
    )
}

struct Classifier {
    macros: Macros,
    tab_width: u32,
}

/// Result of classifying the code part of a line.
type Classified = (Option<Label>, Option<Body>);

impl Classifier {
    fn classify(
        &self,
        raw: &str,
        number: u32,
        diags: &mut Vec<Diagnostic>,
    ) -> (LineContent, BlockRole) {
        if raw.trim().is_empty() {
            return (LineContent::Blank, BlockRole::None);
        }
        let original_column = self.visual_indent(raw);
        let (code, comment) = match lexer::split_comment(raw) {
            Ok(split) => split,
            Err(e) => {
                diags.push(diag(
                    Severity::Warning,
                    codes::UNTERMINATED_STRING,
                    number,
                    column(raw, e.offset),
                    format!("{}; line left unformatted", e.message),
                ));
                return (
                    LineContent::Verbatim(VerbatimReason::Unparsed),
                    BlockRole::None,
                );
            }
        };
        let comment = comment.map(|c| Comment {
            marker: ";".into(),
            text: c.trim_end().to_string(),
        });
        let lead = code.len() - code.trim_start().len();
        let code = code.trim();
        let parsed = if code.is_empty() {
            Ok((None, None))
        } else {
            self.classify_code(code, original_column, number, lead, raw, diags)
        };
        match parsed {
            Ok((label, body)) => {
                let role = body.as_ref().map_or(BlockRole::None, |b| b.role);
                (
                    LineContent::Code(CodeLine {
                        label,
                        body,
                        comment,
                        original_column,
                    }),
                    role,
                )
            }
            Err(e) => {
                diags.push(diag(
                    Severity::Warning,
                    codes::UNPARSED,
                    number,
                    column(raw, lead + e.offset),
                    format!("{}; line left unformatted", e.message),
                ));
                (
                    LineContent::Verbatim(VerbatimReason::Unparsed),
                    BlockRole::None,
                )
            }
        }
    }

    fn visual_indent(&self, raw: &str) -> u32 {
        let mut col = 0;
        for c in raw.chars() {
            match c {
                ' ' => col += 1,
                '\t' => col += self.tab_width - col % self.tab_width,
                _ => break,
            }
        }
        col
    }

    fn classify_code(
        &self,
        code: &str,
        original_column: u32,
        number: u32,
        lead: usize,
        raw: &str,
        diags: &mut Vec<Diagnostic>,
    ) -> Result<Classified, LexError> {
        let bytes = code.as_bytes();
        if bytes[0] == b'%' && bytes.get(1).is_some_and(u8::is_ascii_alphabetic) {
            return Ok((None, Some(preprocessor(code))));
        }
        if bytes[0] == b'[' {
            return bracketed(code).map(|b| (None, Some(b)));
        }
        self.statement(code, original_column, number, lead, raw, diags)
    }

    fn is_statement_start(&self, code: &str, tok: Option<&Tok>) -> bool {
        let Some(tok) = tok else { return false };
        if tok.kind != TokenKind::Identifier {
            return false;
        }
        let word = &code[tok.start..tok.end];
        matches!(
            keyword(word),
            Some(Kw::Mnemonic | Kw::Prefix | Kw::Data | Kw::Equ | Kw::Times | Kw::Struc(_))
        ) || self.macros.contains(word)
    }

    fn statement(
        &self,
        code: &str,
        original_column: u32,
        number: u32,
        lead: usize,
        raw: &str,
        diags: &mut Vec<Diagnostic>,
    ) -> Result<Classified, LexError> {
        let (toks, err) = lexer::lex(code);
        let text = |t: &Tok| &code[t.start..t.end];
        let fail = |offset: usize, message: &str| LexError {
            offset,
            message: message.into(),
        };
        if toks.is_empty() {
            return Err(err.unwrap_or_else(|| fail(0, "empty statement")));
        }

        // Label (NASM.md §4 steps 4 and 9).
        let labelable = |t: &Tok| match t.kind {
            TokenKind::Identifier => !reserved(text(t)),
            // `%%x`, `%$x`, `%1`, and pasted names like `start%1`.
            TokenKind::MacroParam => true,
            _ => false,
        };
        let mut i = 0;
        let mut label = None;
        let make_label = |t: &Tok, colon| Label {
            name: text(t).to_string(),
            kind: label_kind(text(t)),
            colon,
        };
        if labelable(&toks[0]) && toks.get(1).is_some_and(|t| t.kind == TokenKind::Colon) {
            label = Some(make_label(&toks[0], true));
            i = 2;
        } else if labelable(&toks[0]) && !self.macros.contains(text(&toks[0])) {
            let alone = toks.len() == 1 && err.is_none();
            // A lone word at column 0 is a label; indented, it is more
            // likely a macro call without arguments (ADR-0019).
            if (alone && original_column == 0) || self.is_statement_start(code, toks.get(1)) {
                label = Some(make_label(&toks[0], false));
                i = 1;
            }
        }
        if i >= toks.len() {
            return match err {
                Some(e) => Err(e),
                None => Ok((label, None)),
            };
        }

        // Prefixes, including `times <expr>`.
        let mut prefixes = Vec::new();
        let mut j = i;
        while let Some(tok) = toks.get(j) {
            let next_is_insn = toks.get(j + 1).is_some_and(|n| {
                n.kind == TokenKind::Identifier
                    && matches!(
                        keyword(text(n)),
                        Some(Kw::Mnemonic | Kw::Prefix | Kw::SegmentRegister)
                    )
            });
            match tok.kind {
                TokenKind::Decorator if j + 1 < toks.len() => {
                    prefixes.push(token(code, tok, TokenKind::Decorator));
                    j += 1;
                }
                TokenKind::Identifier if text(tok).eq_ignore_ascii_case("times") => {
                    let stop = toks[j + 1..].iter().position(|t| {
                        self.is_statement_start(code, Some(t))
                            && !text(t).eq_ignore_ascii_case("times")
                    });
                    let Some(stop) = stop.map(|p| p + j + 1) else {
                        // `times` with no recognizable keyword: lay out
                        // `times` as the keyword and the rest as operands.
                        let body = self.body(
                            code,
                            &toks,
                            j,
                            &err,
                            BodyKind::Data,
                            TokenKind::DataKeyword,
                            BlockRole::None,
                            prefixes,
                        )?;
                        return Ok((label, Some(body)));
                    };
                    prefixes.push(token(code, tok, TokenKind::DataKeyword));
                    prefixes.extend(
                        toks[j + 1..stop]
                            .iter()
                            .map(|t| token(code, t, operand_kind(t, text(t)))),
                    );
                    j = stop;
                }
                TokenKind::Identifier if contains(tables::PREFIXES, text(tok)) && next_is_insn => {
                    prefixes.push(token(code, tok, TokenKind::Prefix));
                    j += 1;
                }
                TokenKind::Identifier
                    if contains(tables::SEGMENT_REGISTERS, text(tok)) && next_is_insn =>
                {
                    prefixes.push(token(code, tok, TokenKind::Register));
                    j += 1;
                }
                _ => break,
            }
        }
        let Some(kw_tok) = toks.get(j) else {
            return Err(
                err.unwrap_or_else(|| fail(code.len(), "expected an instruction after the prefix"))
            );
        };

        let (kind, kw_kind, role) = match kw_tok.kind {
            TokenKind::Identifier => match keyword(text(kw_tok)) {
                Some(Kw::Mnemonic) => (BodyKind::Instruction, TokenKind::Mnemonic, BlockRole::None),
                Some(Kw::Prefix) => (BodyKind::Instruction, TokenKind::Prefix, BlockRole::None),
                Some(Kw::Data | Kw::Times) => {
                    (BodyKind::Data, TokenKind::DataKeyword, BlockRole::None)
                }
                Some(Kw::Equ) => (BodyKind::Equ, TokenKind::DataKeyword, BlockRole::None),
                Some(Kw::Struc(role)) => (BodyKind::Struc, TokenKind::Directive, role),
                Some(Kw::Section) => (BodyKind::Section, TokenKind::Directive, BlockRole::None),
                Some(Kw::Directive { .. }) => {
                    (BodyKind::Directive, TokenKind::Directive, BlockRole::None)
                }
                Some(Kw::SegmentRegister) => {
                    return Err(fail(
                        kw_tok.start,
                        "segment register without an instruction",
                    ));
                }
                None => {
                    if !self.macros.contains(text(kw_tok)) {
                        diags.push(diag(
                            Severity::Note,
                            codes::ASSUMED_MACRO,
                            number,
                            column(raw, lead + kw_tok.start),
                            format!(
                                "`{}` is not a known instruction; treating it as a macro call",
                                text(kw_tok)
                            ),
                        ));
                    }
                    (BodyKind::MacroCall, TokenKind::Identifier, BlockRole::None)
                }
            },
            TokenKind::MacroParam => (BodyKind::MacroCall, TokenKind::MacroParam, BlockRole::None),
            _ => {
                return Err(fail(
                    kw_tok.start,
                    "expected an instruction, directive or label",
                ));
            }
        };
        let body = self.body(code, &toks, j, &err, kind, kw_kind, role, prefixes)?;
        Ok((label, Some(body)))
    }

    /// Builds a body whose keyword is `toks[j]`; the rest become operands.
    #[allow(clippy::too_many_arguments)]
    fn body(
        &self,
        code: &str,
        toks: &[Tok],
        j: usize,
        err: &Option<LexError>,
        kind: BodyKind,
        kw_kind: TokenKind,
        role: BlockRole,
        prefixes: Vec<Token>,
    ) -> Result<Body, LexError> {
        let kw = &toks[j];
        if let Some(e) = err
            && e.offset < kw.end
        {
            return Err(e.clone());
        }
        let rest = code[kw.end..].trim();
        let operands = if kind == BodyKind::MacroCall || err.is_some() {
            Operands::Raw(rest.to_string())
        } else {
            operand_tokens(code, &toks[j + 1..])
        };
        Ok(Body {
            kind,
            prefixes,
            keyword: token(code, kw, kw_kind),
            operands,
            role,
            bracketed: false,
        })
    }
}

fn preprocessor(code: &str) -> Body {
    let end = code[1..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map_or(code.len(), |p| p + 1);
    let name = &code[..end];
    let lower = name.to_ascii_lowercase();
    let known = contains(tables::PREPROCESSOR_DIRECTIVES, &lower);
    Body {
        kind: BodyKind::Preprocessor,
        prefixes: Vec::new(),
        keyword: Token {
            kind: if known {
                TokenKind::PreprocessorName
            } else {
                TokenKind::MacroParam
            },
            text: name.to_string(),
            space_before: false,
        },
        operands: Operands::Raw(code[end..].trim().to_string()),
        role: if known {
            pp_role(&lower)
        } else {
            BlockRole::None
        },
        bracketed: false,
    }
}

fn bracketed(code: &str) -> Result<Body, LexError> {
    let inner = code
        .strip_prefix('[')
        .and_then(|c| c.strip_suffix(']'))
        .ok_or_else(|| LexError {
            offset: code.len(),
            message: "expected `]` at the end of the directive".into(),
        })?;
    let shift = |mut e: LexError| {
        e.offset += 1;
        e
    };
    let (toks, err) = lexer::lex(inner);
    if let Some(e) = err {
        return Err(shift(e));
    }
    let Some(first) = toks.first().filter(|t| t.kind == TokenKind::Identifier) else {
        return Err(LexError {
            offset: 1,
            message: "expected a directive name after `[`".into(),
        });
    };
    let name = &inner[first.start..first.end];
    let kind = if contains(tables::SECTION_DIRECTIVES, name) {
        BodyKind::Section
    } else {
        BodyKind::Directive
    };
    let keyword_kind = if keyword(name).is_some() {
        TokenKind::Directive
    } else {
        TokenKind::Identifier
    };
    Ok(Body {
        kind,
        prefixes: Vec::new(),
        keyword: token(inner, first, keyword_kind),
        operands: operand_tokens(inner, &toks[1..]),
        role: BlockRole::None,
        bracketed: true,
    })
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
