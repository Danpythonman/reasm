//! Column placement (ARCHITECTURE §5.2, ADR-0010). Works on cell texts and
//! widths only, so it is shared by every dialect.

use super::normalize::{CodeRow, CommentPlace, OperandGap, Row};
use crate::config::{AlignBreak, ColumnAlign, Config, InlineLabelAlign};
use crate::ir::BlockRole;

/// Where each cell of a code row starts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Columns {
    pub label: u32,
    pub keyword: u32,
    pub operands: u32,
    pub comment: u32,
}

/// The visual column after writing `text` starting at `col`.
pub fn advance(col: u32, text: &str, tab_width: u32) -> u32 {
    text.chars().fold(col, |c, ch| {
        if ch == '\t' {
            c + tab_width - c % tab_width
        } else {
            c + 1
        }
    })
}

/// Computes columns for every row. Comment rows get their column in
/// `Columns::comment`; blank and verbatim rows get defaults.
pub fn layout(rows: &[Row<'_>], cfg: &Config) -> Vec<Columns> {
    let tw = cfg.indent.tab_width.max(1);
    let groups = groups(rows, cfg);
    let mut cols = vec![Columns::default(); rows.len()];
    let cap = cfg.alignment.max_column;
    let code = |i: usize| match &rows[i] {
        Row::Code(c) => Some(c),
        _ => None,
    };

    // Block alignment: rows aligned together share the largest column any
    // of them needs, unless that is past `max_column`.
    let align = |members: &[usize], need: &dyn Fn(usize) -> Option<u32>, out: &mut Vec<u32>| {
        let target = members
            .iter()
            .filter_map(|&i| need(i))
            .filter(|&n| n <= cap)
            .max();
        for &i in members {
            if let Some(n) = need(i) {
                out[i] = match target {
                    Some(t) if n <= cap => t,
                    _ => n,
                };
            }
        }
    };

    // 1. Labels and keyword columns.
    let label_end = |i: usize| {
        code(i)
            .and_then(|c| c.label.as_ref())
            .map(|l| advance(l.column, &l.text, tw))
    };
    let inline_need = |i: usize| {
        let c = code(i)?;
        let kw = c.keyword.as_ref()?;
        let end = label_end(i)?;
        Some(kw.column.max(end + cfg.labels.inline_gap))
    };
    let mut kw_col: Vec<u32> = (0..rows.len())
        .map(|i| {
            inline_need(i)
                .or_else(|| code(i)?.keyword.as_ref().map(|k| k.column))
                .unwrap_or(0)
        })
        .collect();
    if cfg.instructions.inline_label_align == InlineLabelAlign::Block {
        for g in &groups {
            align(g, &inline_need, &mut kw_col);
        }
    }

    // 2. Operands.
    let kw_end = |i: usize, kw_col: &[u32]| {
        code(i)
            .and_then(|c| c.keyword.as_ref())
            .map(|k| advance(kw_col[i], &k.text, tw))
    };
    let ins = &cfg.instructions;
    let mut op_col = vec![0u32; rows.len()];
    for i in 0..rows.len() {
        let Some(c) = code(i) else { continue };
        let (Some(kw), Some(_)) = (&c.keyword, &c.operands) else {
            continue;
        };
        let end = kw_end(i, &kw_col).unwrap_or(0);
        op_col[i] = match (kw.gap, ins.operand_align) {
            (OperandGap::Spaces(n), _) => end + n,
            (OperandGap::Instruction, ColumnAlign::Fixed) => {
                (kw_col[i] + ins.operand_offset).max(end + ins.min_gap)
            }
            (OperandGap::Instruction, _) => end + ins.min_gap,
        };
    }
    if ins.operand_align == ColumnAlign::Block {
        let need = |i: usize| {
            let c = code(i)?;
            let kw = c.keyword.as_ref()?;
            c.operands.as_ref()?;
            (kw.gap == OperandGap::Instruction)
                .then(|| kw_end(i, &kw_col).unwrap_or(0) + ins.min_gap)
        };
        for g in &groups {
            align(g, &need, &mut op_col);
        }
    }

    // 3. Trailing comments.
    let code_end = |i: usize, c: &CodeRow| {
        if let Some(ops) = &c.operands {
            advance(op_col[i], ops, tw)
        } else if let Some(end) = kw_end(i, &kw_col) {
            end
        } else {
            label_end(i).unwrap_or(0)
        }
    };
    let cm = &cfg.comments;
    let mut c_col = vec![0u32; rows.len()];
    let comment_need = |i: usize| {
        let c = code(i)?;
        c.comment.as_ref()?;
        Some(code_end(i, c) + cm.min_gap)
    };
    for (i, col) in c_col.iter_mut().enumerate() {
        if let Some(n) = comment_need(i) {
            *col = match cm.align {
                ColumnAlign::None => n,
                ColumnAlign::Fixed | ColumnAlign::Block => n.max(cm.column),
            };
        }
    }
    if cm.align == ColumnAlign::Block {
        let need = |i: usize| comment_need(i).map(|n| n.max(cm.column));
        for g in &groups {
            align(g, &need, &mut c_col);
        }
    }

    for i in 0..rows.len() {
        if let Some(c) = code(i) {
            cols[i] = Columns {
                label: c.label.as_ref().map_or(0, |l| l.column),
                keyword: kw_col[i],
                operands: op_col[i],
                comment: c_col[i],
            };
        }
    }

    // 4. Comment-only rows that follow the code's indent.
    let first_col = |i: usize| {
        code(i).map(|c| match &c.label {
            Some(l) => l.column,
            None => kw_col[i],
        })
    };
    let mut next = None;
    for i in (0..rows.len()).rev() {
        if let Some(col) = first_col(i) {
            next = Some(col);
        }
        if let Row::Comment { place, .. } = &rows[i] {
            cols[i].comment = match place {
                CommentPlace::Column(c) => *c,
                CommentPlace::FollowCode => next.unwrap_or(u32::MAX),
            };
        }
    }
    // Comments after the last code row follow the previous code row.
    let mut prev = 0;
    for (i, c) in cols.iter_mut().enumerate() {
        if let Some(col) = first_col(i) {
            prev = col;
        }
        if c.comment == u32::MAX {
            c.comment = prev;
        }
    }
    cols
}

/// Splits rows into alignment groups (FMT-15). Only code rows are
/// members; any other row may end the current group.
fn groups(rows: &[Row<'_>], cfg: &Config) -> Vec<Vec<usize>> {
    let on = |b: AlignBreak| cfg.alignment.break_on.contains(&b);
    let mut groups: Vec<Vec<usize>> = vec![Vec::new()];
    let mut prev: Option<&CodeRow> = None;
    let brk = |groups: &mut Vec<Vec<usize>>| {
        if groups.last().is_some_and(|g| !g.is_empty()) {
            groups.push(Vec::new());
        }
    };
    for (i, row) in rows.iter().enumerate() {
        match row {
            Row::Blank => {
                if on(AlignBreak::Blank) {
                    brk(&mut groups);
                }
            }
            Row::Verbatim { .. } => brk(&mut groups),
            Row::Comment { .. } => {
                if on(AlignBreak::Comment) {
                    brk(&mut groups);
                }
            }
            Row::Code(c) => {
                let label_only = c.label.is_some() && c.keyword.is_none();
                let block_change = c.role != BlockRole::None
                    || prev.is_some_and(|p| p.depth != c.depth || p.role != BlockRole::None);
                if (label_only && on(AlignBreak::Label))
                    || (block_change && on(AlignBreak::Block))
                    || (c.directive && on(AlignBreak::Directive))
                    || prev.is_some_and(|p| p.directive && on(AlignBreak::Directive))
                {
                    brk(&mut groups);
                }
                groups.last_mut().expect("never empty").push(i);
                prev = Some(c);
            }
        }
    }
    groups
}
