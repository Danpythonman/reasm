//! The dialect-neutral intermediate representation (ARCHITECTURE §3,
//! ADR-0003). A [`Document`] has exactly one [`Line`] per source line.

/// A parsed file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document<'a> {
    pub lines: Vec<Line<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line<'a> {
    /// 1-based source line number.
    pub number: u32,
    /// Block nesting *at* this line. Openers, closers and middles (`%else`)
    /// carry the depth outside their block; body lines are one deeper.
    pub depth: BlockDepth,
    pub content: LineContent<'a>,
}

/// Nesting depth per block kind, so each kind gets its own indent (FMT-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockDepth {
    pub macro_def: u16,
    pub conditional: u16,
    pub rep: u16,
    pub struc: u16,
}

impl BlockDepth {
    pub fn get(&self, kind: BlockKind) -> u16 {
        match kind {
            BlockKind::Macro => self.macro_def,
            BlockKind::Conditional => self.conditional,
            BlockKind::Rep => self.rep,
            BlockKind::Struc => self.struc,
        }
    }

    pub fn get_mut(&mut self, kind: BlockKind) -> &mut u16 {
        match kind {
            BlockKind::Macro => &mut self.macro_def,
            BlockKind::Conditional => &mut self.conditional,
            BlockKind::Rep => &mut self.rep,
            BlockKind::Struc => &mut self.struc,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Macro,
    Conditional,
    Rep,
    Struc,
}

/// How a line takes part in block structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockRole {
    None,
    Open(BlockKind),
    /// `%elif`, `%else`: closes one branch and opens the next.
    Middle(BlockKind),
    Close(BlockKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineContent<'a> {
    Blank,
    /// Output as written (ADR-0006, ADR-0017).
    Verbatim(VerbatimReason),
    Code(CodeLine<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbatimReason {
    /// The line couldn't be parsed; a warning was emitted.
    Unparsed,
    /// Part of a `\` line-continuation group.
    Continuation,
    /// Inside `reasm: off` … `reasm: on`, or marked `reasm: skip`. Kept
    /// byte for byte, trailing whitespace included.
    Disabled,
    /// Inside `%if 0` … `%endif`.
    CommentedOut,
}

/// A line with code and/or a comment. A comment-only line has neither
/// `label` nor `body`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLine<'a> {
    pub label: Option<Label<'a>>,
    pub body: Option<Body<'a>>,
    pub comment: Option<Comment<'a>>,
    /// Visual column (tabs expanded) where the line's content started.
    pub original_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label<'a> {
    /// Exactly as written, without the colon.
    pub name: &'a str,
    pub kind: LabelKind,
    pub colon: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    NonLocal,
    /// `.name`
    Local,
    /// `..@name`, `..start`
    Special,
    /// `%%name`
    MacroLocal,
    /// `%$name`
    ContextLocal,
}

impl LabelKind {
    /// Whether `[labels] local_indent` (rather than `indent`) applies.
    pub fn is_local(self) -> bool {
        !matches!(self, Self::NonLocal)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body<'a> {
    pub kind: BodyKind,
    /// `lock`, `rep`, segment prefixes, `{vex}`, and `times <expr>`, in order.
    pub prefixes: Vec<Token<'a>>,
    /// The mnemonic, data keyword, directive, preprocessor directive or
    /// macro name.
    pub keyword: Token<'a>,
    pub operands: Operands<'a>,
    pub role: BlockRole,
    /// Written in primitive `[...]` form (`[bits 64]`, `[section .text]`);
    /// the brackets are implied around `keyword` and `operands`.
    pub bracketed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Instruction,
    /// `db`, `resb`, `incbin`, `align`, and `times` with no recognizable keyword.
    Data,
    Equ,
    /// `section`, `segment`.
    Section,
    /// Other directives: `global`, `extern`, `bits`, `default`, `org`, ...
    Directive,
    /// `struc`, `endstruc`, `istruc`, `at`, `iend`.
    Struc,
    /// `%define`, `%macro`, `%if`, ...
    Preprocessor,
    /// A user macro invocation (or an unknown word at statement start).
    MacroCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operands<'a> {
    /// Fully lexed: spacing may be normalized.
    Tokens(Vec<Token<'a>>),
    /// Not lexed (macro arguments, preprocessor bodies) or not lexable:
    /// laid out as one unit, text untouched (ADR-0006 level 2).
    Raw(&'a str),
}

impl Operands<'_> {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Tokens(t) => t.is_empty(),
            Self::Raw(s) => s.is_empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: TokenKind,
    /// Exactly as written.
    pub text: &'a str,
    /// Whether whitespace preceded the token in the source.
    pub space_before: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    // Case-transformable kinds (ADR-0008): only set when the word is in
    // the dialect's keyword table.
    Mnemonic,
    Prefix,
    Register,
    SizeKeyword,
    /// `strict`, `rel`, `near`, ...
    OperandKeyword,
    DataKeyword,
    Directive,
    PreprocessorName,
    // Never case-transformed.
    Identifier,
    Number,
    String,
    /// `%1`, `%%x`, `%$x`, `%{...}`, `%[...]`, `%+`, ...
    MacroParam,
    /// `{k1}`, `{z}`, `{1to16}`, `{vex}`: kept as one token.
    Decorator,
    /// `$` and `$$`.
    Location,
    Comma,
    OpenBracket,
    CloseBracket,
    OpenParen,
    CloseParen,
    Plus,
    Minus,
    Star,
    Colon,
    /// Any other operator: `<<`, `//`, `==`, `?`, ...
    Operator,
}

impl TokenKind {
    pub fn is_case_transformable(self) -> bool {
        matches!(
            self,
            Self::Mnemonic
                | Self::Prefix
                | Self::Register
                | Self::SizeKeyword
                | Self::OperandKeyword
                | Self::DataKeyword
                | Self::Directive
                | Self::PreprocessorName
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment<'a> {
    /// The comment marker: `;` for NASM (EXT-2).
    pub marker: &'a str,
    /// Everything after the marker, as written (trailing whitespace trimmed).
    pub text: &'a str,
}

/// A token as seen by the safety check (ADR-0011). Dialects produce these
/// with [`crate::dialect::Dialect::check_lines`]; whitespace between
/// tokens is not represented, so it may change freely.
/// The safety-check tokens of a whole text: line `i` has
/// `tokens[starts[i]..starts[i + 1]]` (the last line runs to the end).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckLines<'a> {
    pub tokens: Vec<CheckToken<'a>>,
    pub starts: Vec<usize>,
}

impl<'a> CheckLines<'a> {
    pub fn len(&self) -> usize {
        self.starts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.starts.is_empty()
    }

    pub fn line(&self, i: usize) -> &[CheckToken<'a>] {
        let end = self.starts.get(i + 1).copied().unwrap_or(self.tokens.len());
        &self.tokens[self.starts[i]..end]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckToken<'a> {
    pub kind: CheckKind,
    /// A slice of the checked text.
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    /// A name or keyword. It may change case only if
    /// [`crate::dialect::Dialect::case_insensitive`] says so.
    Word,
    /// `:` (may be added after a label, `labels.add_colon`).
    Colon,
    /// A comment; `text` is everything after the marker. Only the
    /// whitespace around the text may change.
    Comment,
    /// Anything else, compared exactly.
    Other,
}
