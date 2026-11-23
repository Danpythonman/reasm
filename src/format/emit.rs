//! Rows + columns → text (ARCHITECTURE §5.3): blank-line limits,
//! indentation style, and joining the cells.

use super::layout::{Columns, advance};
use super::normalize::Row;
use crate::config::{Config, IndentStyle};

/// Writes one line into the output, padding to each cell's column.
/// Leading indentation uses tabs when `indent.style = "tabs"`; padding
/// inside the line is always spaces (ADR-0015).
struct LineBuf<'o, 'c> {
    out: &'o mut String,
    /// Whether nothing has been written on this line yet.
    empty: bool,
    col: u32,
    cfg: &'c Config,
}

impl LineBuf<'_, '_> {
    fn put(&mut self, col: u32, cell: &str) {
        let tw = self.cfg.indent.tab_width.max(1);
        if self.empty {
            if self.cfg.indent.style == IndentStyle::Tabs {
                self.out
                    .extend(std::iter::repeat_n('\t', (col / tw) as usize));
                pad(self.out, col % tw);
            } else {
                pad(self.out, col);
            }
            self.col = col;
            self.empty = false;
        } else {
            // Layout guarantees col > self.col; one space is the fallback.
            let n = col.saturating_sub(self.col).max(1);
            pad(self.out, n);
            self.col += n;
        }
        self.out.push_str(cell);
        self.col = advance(self.col, cell, tw);
    }
}

fn pad(s: &mut String, n: u32) {
    s.extend(std::iter::repeat_n(' ', n as usize));
}

/// Appends the formatted lines to `out`, each followed by `newline`.
pub fn emit(rows: &[Row<'_>], cols: &[Columns], cfg: &Config, newline: &str, out: &mut String) {
    let max_blank = cfg.blank_lines.max_consecutive as usize;
    // Length of `out` after which only droppable blank lines follow.
    let mut keep_len = out.len();
    let mut blanks = 0;
    let mut seen_content = false;
    for (row, col) in rows.iter().zip(cols) {
        match row {
            Row::Blank => {
                blanks += 1;
                if blanks <= max_blank && (seen_content || !cfg.blank_lines.strip_leading) {
                    out.push_str(newline);
                }
                continue;
            }
            Row::Verbatim { text, keep } => {
                out.push_str(text);
                out.push_str(newline);
                if text.is_empty() && !keep {
                    // An empty continuation line: not a blank for the
                    // limits, but droppable at the end of the file.
                    continue;
                }
            }
            Row::Comment { text, .. } => {
                let mut b = LineBuf {
                    out,
                    empty: true,
                    col: 0,
                    cfg,
                };
                b.put(col.comment, text);
                out.push_str(newline);
            }
            Row::Code(c) => {
                let mut b = LineBuf {
                    out,
                    empty: true,
                    col: 0,
                    cfg,
                };
                if let Some(l) = &c.label {
                    b.put(col.label, &l.text);
                }
                if let Some(k) = &c.keyword {
                    b.put(col.keyword, &k.text);
                }
                if let Some(o) = &c.operands {
                    b.put(col.operands, o);
                }
                if let Some(cm) = &c.comment {
                    b.put(col.comment, cm);
                }
                out.push_str(newline);
            }
        }
        blanks = 0;
        seen_content = true;
        keep_len = out.len();
    }
    out.truncate(keep_len);
}
