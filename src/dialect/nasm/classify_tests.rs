//! Classifier tests: one or more per item in docs/NASM.md.

use super::*;
use crate::config::Config;

fn parse_with<'a>(src: &'a str, config: &Config) -> (Document<'a>, Vec<Diagnostic>) {
    let source = Source::new(src);
    let mut diags = Vec::new();
    let doc = parse(&source, config, &mut diags);
    assert_eq!(
        doc.lines.len(),
        source.lines.len(),
        "one IR line per source line"
    );
    (doc, diags)
}

fn doc(src: &str) -> (Document<'_>, Vec<Diagnostic>) {
    parse_with(src, &Config::default())
}

fn code_line<'l, 'a>(line: &'l Line<'a>) -> &'l CodeLine<'a> {
    match &line.content {
        LineContent::Code(c) => c,
        other => panic!("line {} is {other:?}, expected code", line.number),
    }
}

fn one(src: &str) -> CodeLine<'_> {
    let (d, diags) = doc(src);
    assert!(
        diags.iter().all(|d| d.severity == Severity::Note),
        "{src}: {diags:?}"
    );
    code_line(&d.lines[0]).clone()
}

fn body(src: &str) -> Body<'_> {
    one(src).body.unwrap_or_else(|| panic!("{src}: no body"))
}

fn label(src: &str) -> Label<'_> {
    one(src).label.unwrap_or_else(|| panic!("{src}: no label"))
}

fn operand_kinds(b: &Body) -> Vec<(TokenKind, String)> {
    match &b.operands {
        Operands::Tokens(t) => t.iter().map(|t| (t.kind, t.text.to_string())).collect(),
        Operands::Raw(r) => panic!("raw operands: {r:?}"),
    }
}

fn raw_operands<'a>(b: &Body<'a>) -> &'a str {
    match &b.operands {
        Operands::Raw(r) => r,
        Operands::Tokens(t) => panic!("token operands: {t:?}"),
    }
}

fn verbatim(d: &Document, i: usize) -> Option<VerbatimReason> {
    match d.lines[i].content {
        LineContent::Verbatim(r) => Some(r),
        _ => None,
    }
}

fn codes(diags: &[Diagnostic]) -> Vec<&'static str> {
    diags.iter().map(|d| d.code).collect()
}

// --- §3 labels ---------------------------------------------------------------

#[test]
fn label_kinds() {
    use LabelKind::*;
    for (src, name, kind) in [
        ("_start:", "_start", NonLocal),
        (".loop:", ".loop", Local),
        ("..@x:", "..@x", Special),
        ("..start:", "..start", Special),
        ("%%done:", "%%done", MacroLocal),
        ("%$ctx:", "%$ctx", ContextLocal),
        ("$rax:", "$rax", NonLocal),
        ("?q:", "?q", NonLocal),
    ] {
        let l = label(src);
        assert_eq!((l.name, l.kind, l.colon), (name, kind, true), "{src}");
        assert!(one(src).body.is_none());
    }
}

#[test]
fn label_with_instruction() {
    let c = one(".loop: dec rcx");
    assert_eq!(c.label.unwrap().kind, LabelKind::Local);
    let b = c.body.unwrap();
    assert_eq!((b.kind, b.keyword.text), (BodyKind::Instruction, "dec"));
    assert_eq!(operand_kinds(&b), [(TokenKind::Register, "rcx".into())]);
}

#[test]
fn colonless_labels() {
    let c = one("msg db \"hi\", 10");
    assert_eq!(
        c.label.as_ref().map(|l| (l.name, l.colon)),
        Some(("msg", false))
    );
    assert_eq!(c.body.unwrap().kind, BodyKind::Data);

    let c = one("len equ $-msg");
    assert_eq!(c.label.unwrap().name, "len");
    assert_eq!(c.body.unwrap().kind, BodyKind::Equ);

    let c = one("x: equ 5");
    assert!(c.label.unwrap().colon);

    // A lone word at column 0 is an (orphan) label...
    assert_eq!(label("lbl").name, "lbl");
    // ...but indented, it's more likely a macro call without arguments.
    let (d, diags) = doc("    SAVE_REGS");
    assert_eq!(
        code_line(&d.lines[0]).body.as_ref().unwrap().kind,
        BodyKind::MacroCall
    );
    assert_eq!(codes(&diags), ["N001"]);
}

#[test]
fn directive_names_as_labels() {
    // Verified with NASM 2.16.01: plain directives can be labels, but
    // standard macros like `section` expand even before a colon.
    assert_eq!(label("list: db 1").name, "list");
    assert_eq!(label("list db 1").name, "list");
    let c = one("section: db 1");
    assert!(c.label.is_none());
    assert_eq!(c.body.unwrap().kind, BodyKind::Section);
}

#[test]
fn keywords_as_labels_with_a_colon() {
    // Verified with NASM 2.16.01 (test/insnlbl.asm).
    for src in [
        "add: jmp add-2",
        "do: jmp 1",
        "nop: nop",
        "db: nop",
        "resb: nop",
        "equ: nop",
    ] {
        let c = one(src);
        assert!(c.label.as_ref().is_some_and(|l| l.colon), "{src}");
        assert_eq!(c.body.unwrap().kind, BodyKind::Instruction, "{src}");
    }
    for src in [
        "rax: nop",
        "byte: nop",
        "lock: nop",
        "wait: nop",
        "align: nop",
        "rel: nop",
    ] {
        let (d, _) = doc(src);
        let is_label = matches!(&d.lines[0].content, LineContent::Code(c) if c.label.is_some());
        assert!(!is_label, "{src}");
    }
}

// --- §4 classification ------------------------------------------------------------

#[test]
fn directives() {
    let b = body("section .text");
    assert_eq!((b.kind, b.bracketed), (BodyKind::Section, false));
    assert_eq!(body("SEGMENT .data").kind, BodyKind::Section);
    assert_eq!(body("global _start").kind, BodyKind::Directive);
    assert_eq!(body("extern printf").keyword.kind, TokenKind::Directive);
    let b = body("default rel");
    assert_eq!(
        operand_kinds(&b),
        [(TokenKind::OperandKeyword, "rel".into())]
    );

    let b = body("[bits 64]");
    assert_eq!(
        (b.kind, b.bracketed, b.keyword.text),
        (BodyKind::Directive, true, "bits")
    );
    assert_eq!(operand_kinds(&b), [(TokenKind::Number, "64".into())]);
    let b = body("[section .data]");
    assert_eq!((b.kind, b.bracketed), (BodyKind::Section, true));
}

#[test]
fn data_keywords() {
    for src in [
        "db 1",
        "DW 1",
        "resb 64",
        "incbin \"f.bin\"",
        "align 16",
        "alignb 4",
        "dq 1.5e+3",
    ] {
        assert_eq!(body(src).kind, BodyKind::Data, "{src}");
    }
}

#[test]
fn prefixes() {
    let prefix_texts = |b: &Body| {
        b.prefixes
            .iter()
            .map(|t| t.text.to_string())
            .collect::<Vec<_>>()
    };

    let b = body("lock add [rax], 1");
    assert_eq!(
        (prefix_texts(&b), b.keyword.text),
        (vec!["lock".to_string()], "add")
    );
    assert_eq!(b.prefixes[0].kind, TokenKind::Prefix);

    let b = body("rep movsb");
    assert_eq!(b.keyword.text, "movsb");

    let b = body("es lodsb");
    assert_eq!(
        (prefix_texts(&b), b.prefixes[0].kind),
        (vec!["es".to_string()], TokenKind::Register)
    );

    // `wait` is both a prefix and an instruction.
    assert_eq!(body("wait").keyword.text, "wait");
    let b = body("wait fnop");
    assert_eq!(
        (prefix_texts(&b), b.keyword.text),
        (vec!["wait".to_string()], "fnop")
    );

    let b = body("{vex} vpdpbusd xmm1, xmm2, xmm3");
    assert_eq!(
        (b.prefixes[0].kind, b.keyword.text),
        (TokenKind::Decorator, "vpdpbusd")
    );
}

#[test]
fn times() {
    let b = body("times 10 db 0");
    assert_eq!(
        b.prefixes.iter().map(|t| t.text).collect::<Vec<_>>(),
        ["times", "10"]
    );
    assert_eq!((b.kind, b.keyword.text), (BodyKind::Data, "db"));

    let b = body("times 510-($-$$) db 0");
    assert_eq!(b.keyword.text, "db");
    assert_eq!(b.prefixes.len(), 8);

    let b = body("times 4 nop");
    assert_eq!((b.kind, b.keyword.text), (BodyKind::Instruction, "nop"));

    // No recognizable keyword: `times` itself is the keyword.
    let (d, _) = doc("times 4 MYMACRO");
    let b = code_line(&d.lines[0]).body.clone().unwrap();
    assert_eq!((b.kind, b.keyword.text), (BodyKind::Data, "times"));
}

#[test]
fn operand_token_kinds() {
    use TokenKind::*;
    let b = body("MOV RAX, QWORD [RBX]");
    assert_eq!(b.keyword.kind, Mnemonic);
    let kinds: Vec<_> = operand_kinds(&b).into_iter().map(|(k, _)| k).collect();
    assert_eq!(
        kinds,
        [
            Register,
            Comma,
            SizeKeyword,
            OpenBracket,
            Register,
            CloseBracket
        ]
    );

    // `$eax` is a symbol escape; `rip` is not a NASM register (NASM.md §5).
    let b = body("mov eax, $eax");
    assert_eq!(operand_kinds(&b)[2], (Identifier, "$eax".into()));
    let b = body("lea rax, [rip+8]");
    assert_eq!(operand_kinds(&b)[3], (Identifier, "rip".into()));

    let b = body("jmp short .x");
    assert_eq!(operand_kinds(&b)[0], (OperandKeyword, "short".into()));

    let b = body("vaddps zmm1{k1}{z}, zmm2, [rax]{1to16}");
    let ops = operand_kinds(&b);
    assert_eq!(ops[1], (Decorator, "{k1}".into()));
    assert_eq!(ops.last().unwrap(), &(Decorator, "{1to16}".into()));
}

// --- §2 strings and comments ------------------------------------------------------

#[test]
fn comments_and_strings() {
    let c = one("db 'a;b' ; c");
    assert_eq!(c.comment.as_ref().unwrap().text, " c");
    assert_eq!(
        operand_kinds(&c.body.unwrap())[0],
        (TokenKind::String, "'a;b'".into())
    );

    let c = one("   ; hi");
    assert!(c.label.is_none() && c.body.is_none());
    assert_eq!((c.comment.unwrap().text, c.original_column), (" hi", 3));
    assert_eq!(one("\t; x").original_column, 8);
    assert_eq!(one("  \t; x").original_column, 8);

    let (d, diags) = doc("db 'unterminated");
    assert_eq!(verbatim(&d, 0), Some(VerbatimReason::Unparsed));
    assert_eq!(codes(&diags), ["W002"]);
    assert_eq!(diags[0].col, 4);
}

// --- §7 preprocessor and blocks -----------------------------------------------------

#[test]
fn preprocessor_lines() {
    let b = body("%define X 1 + 2");
    assert_eq!(
        (b.kind, b.keyword.kind, b.keyword.text),
        (
            BodyKind::Preprocessor,
            TokenKind::PreprocessorName,
            "%define"
        )
    );
    assert_eq!(raw_operands(&b), "X 1 + 2");
    // Verified with NASM 2.16.01: directive names are case-insensitive.
    assert_eq!(
        body("%DEFINE X 1").keyword.kind,
        TokenKind::PreprocessorName
    );
    assert_eq!(body("%include \"m.inc\" ; x").keyword.text, "%include");
    // Unknown names are kept, but not treated as keywords.
    assert_eq!(body("%frobnicate").keyword.kind, TokenKind::MacroParam);
}

#[test]
fn block_depths() {
    let src = "\
%macro M 1
    mov eax, %1
%%done:
%endmacro
%ifdef X
    nop
%else
    %rep 2
        nop
    %endrep
%endif
struc point
.x resd 1
endstruc
";
    let (d, diags) = doc(src);
    assert!(
        diags.iter().all(|d| d.severity == Severity::Note),
        "{diags:?}"
    );
    let depths: Vec<(u16, u16, u16, u16)> = d
        .lines
        .iter()
        .map(|l| {
            (
                l.depth.macro_def,
                l.depth.conditional,
                l.depth.rep,
                l.depth.struc,
            )
        })
        .collect();
    assert_eq!(
        depths,
        [
            (0, 0, 0, 0), // %macro
            (1, 0, 0, 0),
            (1, 0, 0, 0),
            (0, 0, 0, 0), // %endmacro
            (0, 0, 0, 0), // %ifdef
            (0, 1, 0, 0),
            (0, 0, 0, 0), // %else
            (0, 1, 0, 0), // %rep
            (0, 1, 1, 0),
            (0, 1, 0, 0), // %endrep
            (0, 0, 0, 0), // %endif
            (0, 0, 0, 0), // struc
            (0, 0, 0, 1),
            (0, 0, 0, 0), // endstruc
        ]
    );
    let field = code_line(&d.lines[12]);
    assert_eq!(field.label.as_ref().unwrap().kind, LabelKind::Local);
    assert_eq!(field.body.as_ref().unwrap().kind, BodyKind::Data);
}

#[test]
fn istruc_at_iend() {
    let (d, _) = doc("istruc point\n    at .x, dd 1\niend\n");
    assert_eq!(d.lines[1].depth.struc, 1);
    let at = code_line(&d.lines[1]).body.clone().unwrap();
    assert_eq!((at.kind, at.role), (BodyKind::Struc, BlockRole::None));
    assert_eq!(d.lines[2].depth.struc, 0);
}

#[test]
fn unbalanced_blocks_warn() {
    let (_, diags) = doc("%endif\n");
    assert_eq!(codes(&diags), ["W003"]);
    let (_, diags) = doc("%macro M 0\nnop\n");
    assert_eq!((codes(&diags), diags[0].line), (vec!["W003"], 1));
    let (d, diags) = doc("%macro M 0\n%if 1\n%endmacro\nnop\n");
    assert_eq!(codes(&diags), ["W003"]);
    assert!(diags[0].message.contains("line 2"));
    assert_eq!(d.lines[3].depth, BlockDepth::default());
    let (_, diags) = doc("%else\n");
    assert_eq!(codes(&diags), ["W003"]);
}

#[test]
fn if_zero_is_verbatim() {
    let src = "%if 0\njunk @@ here\n%if X\nmore junk\n%endif\n%else\ndb 1\n%endif\n";
    let (d, diags) = doc(src);
    assert!(diags.is_empty(), "{diags:?}");
    let reasons: Vec<_> = (0..d.lines.len()).map(|i| verbatim(&d, i)).collect();
    use VerbatimReason::CommentedOut as C;
    assert_eq!(
        reasons,
        [None, Some(C), Some(C), Some(C), Some(C), None, None, None]
    );
    assert_eq!(d.lines[1].depth.conditional, 1);
    assert_eq!(d.lines[6].depth.conditional, 1);
    assert_eq!(d.lines[7].depth.conditional, 0);
    // Only the literal condition `0`.
    let (d, _) = doc("%if 1\ndb 1\n%endif\n");
    assert_eq!(verbatim(&d, 1), None);
}

// --- §1 line continuation ------------------------------------------------------------

#[test]
fn continuation_groups_are_verbatim() {
    let (d, diags) = doc("%define X 1 + \\\n  2\nnop\n");
    assert!(diags.is_empty());
    assert_eq!(verbatim(&d, 0), Some(VerbatimReason::Continuation));
    assert_eq!(verbatim(&d, 1), Some(VerbatimReason::Continuation));
    assert_eq!(verbatim(&d, 2), None);
    // Verified with NASM 2.16.01: a comment ending in `\` continues too.
    let (d, _) = doc("db 1 ; note \\\ndb 2\n");
    assert_eq!(verbatim(&d, 1), Some(VerbatimReason::Continuation));
    // `\` followed by spaces is not a continuation.
    let (d, _) = doc("db 1 ; x \\  \ndb 2\n");
    assert_eq!(verbatim(&d, 1), None);
}

// --- ADR-0017 ignore directives ------------------------------------------------------------

#[test]
fn reasm_off_on_skip() {
    let (d, diags) = doc("a:\n; reasm: off\n  x  y z\n;REASM:ON\nb:\n");
    assert!(diags.is_empty());
    assert_eq!(verbatim(&d, 1), None, "markers are formatted");
    assert_eq!(verbatim(&d, 2), Some(VerbatimReason::Disabled));
    assert_eq!(verbatim(&d, 3), None);
    assert_eq!(verbatim(&d, 4), None);

    let (d, _) = doc("mov eax,1 ; reasm: skip\nnop\n");
    assert_eq!(
        (verbatim(&d, 0), verbatim(&d, 1)),
        (Some(VerbatimReason::Disabled), None)
    );
    let (d, _) = doc("; reasm: skip\nmov   eax,1\nnop\n");
    assert_eq!(verbatim(&d, 0), None);
    assert_eq!(
        (verbatim(&d, 1), verbatim(&d, 2)),
        (Some(VerbatimReason::Disabled), None)
    );

    let (_, diags) = doc("; reasm: off\nnop\n");
    assert_eq!(
        (codes(&diags), diags[0].severity),
        (vec!["N002"], Severity::Note)
    );
}

// --- ADR-0006 fallback levels --------------------------------------------------------

#[test]
fn unparsed_lines() {
    let (d, diags) = doc("@foo bar\n");
    assert_eq!(verbatim(&d, 0), Some(VerbatimReason::Unparsed));
    assert_eq!((codes(&diags), diags[0].col), (vec!["W001"], 1));
    let (_, diags) = doc("    es\n");
    assert_eq!(codes(&diags), ["W001"]);
    assert_eq!(diags[0].col, 5);
}

#[test]
fn operand_lex_errors_degrade_to_raw() {
    let (d, diags) = doc("mov eax, \\weird\n");
    assert!(diags.is_empty(), "{diags:?}");
    let b = code_line(&d.lines[0]).body.clone().unwrap();
    assert_eq!((b.keyword.text, raw_operands(&b)), ("mov", "eax, \\weird"));
}

// --- ADR-0019 macros -------------------------------------------------------------------

#[test]
fn macro_calls() {
    let (d, diags) = doc("%macro PRINT 1\n%endmacro\nPRINT msg, 3\n");
    assert!(diags.is_empty(), "{diags:?}");
    let b = code_line(&d.lines[2]).body.clone().unwrap();
    assert_eq!((b.kind, raw_operands(&b)), (BodyKind::MacroCall, "msg, 3"));

    // Macros are case-sensitive; %imacro ones are not.
    let (_, diags) = doc("%macro PRINT 1\n%endmacro\n    print msg\n");
    assert_eq!(codes(&diags), ["N001"]);
    let (_, diags) = doc("%imacro PRINT 1\n%endmacro\n    print msg\n");
    assert!(diags.is_empty());

    // A known macro at column 0 is a call, not a label.
    let (d, _) = doc("%macro EXIT 0\n%endmacro\nEXIT\n");
    assert!(code_line(&d.lines[2]).label.is_none());

    // `label MACRO args` and macros from config.
    let mut config = Config::default();
    config.nasm.known_macros = vec!["SYSCALL3".into()];
    let (d, diags) = parse_with("entry SYSCALL3 1, 2, 3\n", &config);
    assert!(diags.is_empty());
    let c = code_line(&d.lines[0]);
    assert_eq!(c.label.as_ref().unwrap().name, "entry");
    assert_eq!(c.body.as_ref().unwrap().kind, BodyKind::MacroCall);

    // Single-line macros used as statements.
    let (_, diags) = doc("%define SAVE push rax\n    SAVE\n");
    assert!(diags.is_empty());
}

#[test]
fn macro_parameters_in_bodies() {
    let b = body("mov %1, %{2:3}");
    let ops = operand_kinds(&b);
    assert_eq!(ops[0], (TokenKind::MacroParam, "%1".into()));
    assert_eq!(ops[2], (TokenKind::MacroParam, "%{2:3}".into()));
    // A parameter as the instruction.
    let (d, _) = doc("    %1 eax, ebx\n");
    assert_eq!(
        code_line(&d.lines[0]).body.as_ref().unwrap().kind,
        BodyKind::MacroCall
    );
    // Macro-local labels with instructions.
    let c = one("%%loop: dec ecx");
    assert_eq!(c.label.unwrap().kind, LabelKind::MacroLocal);
    // Pasted and parameter labels (seen in NASM's own test suite).
    assert_eq!(label("start%1:").name, "start%1");
    assert_eq!(label("var%1:\tdb 0").name, "var%1");
    assert_eq!(label(".count_%+1:").kind, LabelKind::Local);
    assert_eq!(label("%1:").name, "%1");
    assert!(!label("%1 db 0").colon);
}

#[test]
fn every_line_is_accounted_for() {
    let src = "a:\n\n  ; c\n%if 0\nx\n%endif\n\\\n";
    let (d, _) = doc(src);
    let numbers: Vec<u32> = d.lines.iter().map(|l| l.number).collect();
    assert_eq!(numbers, (1..=7).collect::<Vec<_>>());
}
