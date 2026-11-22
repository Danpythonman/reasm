use serde::{Deserialize, Serialize};

/// Largest column/indent value accepted anywhere, to catch typos like `4000`.
pub const MAX_COLUMN: u32 = 1000;

/// Generates a string-valued config enum with serde names in snake_case.
macro_rules! config_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($(#[$vmeta])* $variant),+ }
    };
}

config_enum!(LineEndingStyle { Preserve, Lf, Crlf });
config_enum!(IndentStyle { Spaces, Tabs });
config_enum!(SplitInline {
    Preserve,
    Code,
    All
});
config_enum!(ColumnAlign { None, Fixed, Block });
config_enum!(InlineLabelAlign { None, Block });
config_enum!(BracketPadding {
    Preserve,
    None,
    Padded
});
config_enum!(OperatorSpacing {
    Preserve,
    Spaced,
    Compact
});
config_enum!(SegmentColon { Preserve, Compact });
config_enum!(ExpressionSpacing { Preserve });
config_enum!(StandaloneComments {
    Preserve,
    Code,
    Zero,
    Smart
});
config_enum!(MarkerSpace { Preserve, Ensure });
config_enum!(AlignBreak {
    Blank,
    Label,
    Comment,
    Block,
    Directive
});
config_enum!(CaseStyle {
    Preserve,
    Lower,
    Upper
});
config_enum!(DirectiveIndent { Zero, Instruction });
config_enum!(PreprocessorIndent {
    Zero,
    Instruction,
    Nested
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub dialect: String,
    pub extensions: Vec<String>,
    pub line_ending: LineEndingStyle,
    pub indent: Indent,
    pub labels: Labels,
    pub instructions: Instructions,
    pub operands: Operands,
    pub comments: Comments,
    pub alignment: Alignment,
    pub case: Case,
    pub blank_lines: BlankLines,
    pub directives: Directives,
    pub preprocessor: Preprocessor,
    pub nasm: NasmOptions,
    pub diagnostics: Diagnostics,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dialect: "auto".into(),
            extensions: vec!["asm".into(), "nasm".into(), "inc".into()],
            line_ending: LineEndingStyle::Preserve,
            indent: Indent::default(),
            labels: Labels::default(),
            instructions: Instructions::default(),
            operands: Operands::default(),
            comments: Comments::default(),
            alignment: Alignment::default(),
            case: Case::default(),
            blank_lines: BlankLines::default(),
            directives: Directives::default(),
            preprocessor: Preprocessor::default(),
            nasm: NasmOptions::default(),
            diagnostics: Diagnostics::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Indent {
    pub style: IndentStyle,
    pub tab_width: u32,
    pub instruction: u32,
}

impl Default for Indent {
    fn default() -> Self {
        Self {
            style: IndentStyle::Spaces,
            tab_width: 8,
            instruction: 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Labels {
    pub indent: u32,
    pub local_indent: u32,
    pub split_inline: SplitInline,
    pub add_colon: bool,
    pub add_colon_to_equ: bool,
    pub inline_gap: u32,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            indent: 0,
            local_indent: 0,
            split_inline: SplitInline::All,
            add_colon: false,
            add_colon_to_equ: false,
            inline_gap: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Instructions {
    pub operand_align: ColumnAlign,
    pub operand_offset: u32,
    pub min_gap: u32,
    pub inline_label_align: InlineLabelAlign,
}

impl Default for Instructions {
    fn default() -> Self {
        Self {
            operand_align: ColumnAlign::None,
            operand_offset: 8,
            min_gap: 1,
            inline_label_align: InlineLabelAlign::Block,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Operands {
    pub space_after_comma: bool,
    pub space_before_comma: bool,
    pub bracket_padding: BracketPadding,
    pub memory_plus_minus: OperatorSpacing,
    pub memory_scale: OperatorSpacing,
    pub segment_colon: SegmentColon,
    pub size_keyword_space: bool,
    pub expressions: ExpressionSpacing,
}

impl Default for Operands {
    fn default() -> Self {
        Self {
            space_after_comma: true,
            space_before_comma: false,
            bracket_padding: BracketPadding::Preserve,
            memory_plus_minus: OperatorSpacing::Preserve,
            memory_scale: OperatorSpacing::Preserve,
            segment_colon: SegmentColon::Preserve,
            size_keyword_space: true,
            expressions: ExpressionSpacing::Preserve,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Comments {
    pub align: ColumnAlign,
    pub column: u32,
    pub min_gap: u32,
    pub standalone: StandaloneComments,
    pub space_after_marker: MarkerSpace,
}

impl Default for Comments {
    fn default() -> Self {
        Self {
            align: ColumnAlign::Fixed,
            column: 39,
            min_gap: 1,
            standalone: StandaloneComments::Zero,
            space_after_marker: MarkerSpace::Ensure,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Alignment {
    pub break_on: Vec<AlignBreak>,
    pub max_column: u32,
}

impl Default for Alignment {
    fn default() -> Self {
        Self {
            break_on: vec![AlignBreak::Blank, AlignBreak::Label, AlignBreak::Block],
            max_column: 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Case {
    pub mnemonic: CaseStyle,
    pub register: CaseStyle,
    pub size_keyword: CaseStyle,
    pub data: CaseStyle,
    pub directive: CaseStyle,
    pub preprocessor: CaseStyle,
}

impl Default for Case {
    fn default() -> Self {
        Self {
            mnemonic: CaseStyle::Preserve,
            register: CaseStyle::Preserve,
            size_keyword: CaseStyle::Preserve,
            data: CaseStyle::Preserve,
            directive: CaseStyle::Preserve,
            preprocessor: CaseStyle::Preserve,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BlankLines {
    pub max_consecutive: u32,
    pub strip_leading: bool,
}

impl Default for BlankLines {
    fn default() -> Self {
        Self {
            max_consecutive: 1,
            strip_leading: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Directives {
    pub section_indent: DirectiveIndent,
    pub indent: DirectiveIndent,
    pub arg_gap: u32,
}

impl Default for Directives {
    fn default() -> Self {
        Self {
            section_indent: DirectiveIndent::Zero,
            indent: DirectiveIndent::Instruction,
            arg_gap: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preprocessor {
    pub indent: PreprocessorIndent,
    pub arg_gap: u32,
    pub macro_body_indent: u32,
    pub conditional_body_indent: u32,
    pub rep_body_indent: u32,
    pub struc_body_indent: u32,
    pub format_macro_args: bool,
}

impl Default for Preprocessor {
    fn default() -> Self {
        Self {
            indent: PreprocessorIndent::Zero,
            arg_gap: 1,
            macro_body_indent: 0,
            conditional_body_indent: 0,
            rep_body_indent: 0,
            struc_body_indent: 0,
            format_macro_args: false,
        }
    }
}

/// The `[nasm]` table. Each dialect owns one table like this (CFG-5).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NasmOptions {
    pub known_macros: Vec<String>,
    pub known_imacros: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Diagnostics {
    pub allow: Vec<String>,
    pub deny_warnings: bool,
}

impl Config {
    /// The configuration as TOML, in documentation order (`--print-config`).
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("Config always serializes")
    }

    /// Checks value ranges and cross-field rules that serde can't express.
    /// Returns a message naming the offending key.
    pub fn validate(&self) -> Result<(), String> {
        let dialect_ok = self.dialect == "auto" || crate::dialect::by_name(&self.dialect).is_some();
        if !dialect_ok {
            return Err(format!(
                "`dialect`: unknown dialect `{}` (expected \"auto\" or one of: {})",
                self.dialect,
                crate::dialect::names().join(", ")
            ));
        }
        for ext in &self.extensions {
            if ext.is_empty() || ext.starts_with('.') || ext.contains(['/', '\\']) {
                return Err(format!(
                    "`extensions`: invalid extension `{ext}` (write it without the dot, e.g. \"asm\")"
                ));
            }
        }

        let ranges = [
            ("indent.tab_width", self.indent.tab_width, 1, 16),
            ("indent.instruction", self.indent.instruction, 0, MAX_COLUMN),
            ("labels.indent", self.labels.indent, 0, MAX_COLUMN),
            (
                "labels.local_indent",
                self.labels.local_indent,
                0,
                MAX_COLUMN,
            ),
            ("labels.inline_gap", self.labels.inline_gap, 1, MAX_COLUMN),
            (
                "instructions.operand_offset",
                self.instructions.operand_offset,
                0,
                MAX_COLUMN,
            ),
            (
                "instructions.min_gap",
                self.instructions.min_gap,
                1,
                MAX_COLUMN,
            ),
            ("comments.column", self.comments.column, 0, MAX_COLUMN),
            ("comments.min_gap", self.comments.min_gap, 1, MAX_COLUMN),
            (
                "alignment.max_column",
                self.alignment.max_column,
                0,
                MAX_COLUMN,
            ),
            ("directives.arg_gap", self.directives.arg_gap, 1, MAX_COLUMN),
            (
                "preprocessor.arg_gap",
                self.preprocessor.arg_gap,
                1,
                MAX_COLUMN,
            ),
            (
                "preprocessor.macro_body_indent",
                self.preprocessor.macro_body_indent,
                0,
                MAX_COLUMN,
            ),
            (
                "preprocessor.conditional_body_indent",
                self.preprocessor.conditional_body_indent,
                0,
                MAX_COLUMN,
            ),
            (
                "preprocessor.rep_body_indent",
                self.preprocessor.rep_body_indent,
                0,
                MAX_COLUMN,
            ),
            (
                "preprocessor.struc_body_indent",
                self.preprocessor.struc_body_indent,
                0,
                MAX_COLUMN,
            ),
        ];
        for (key, value, min, max) in ranges {
            if value < min || value > max {
                return Err(format!("`{key}`: {value} is out of range ({min}..={max})"));
            }
        }

        // NASM.md §7: until NASM's case-insensitivity for `%` directives is
        // verified, changing their case could break code.
        if self.case.preprocessor != CaseStyle::Preserve {
            return Err(
                "`case.preprocessor`: only \"preserve\" is supported for now \
                        (see docs/NASM.md §7)"
                    .into(),
            );
        }
        Ok(())
    }
}
