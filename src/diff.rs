//! Unified line diffs for `--diff` (CLI-6, ADR-0023).
//!
//! Patience-style anchoring on lines that occur once on each side splits the
//! input into small gaps, and each gap is diffed with an LCS table. A gap too
//! large for the table is shown as a block replacement, which is still a
//! correct (if less minimal) diff. This keeps a whole reformatted file fast
//! without a diff dependency (ADR-0012).

use std::collections::HashMap;
use std::fmt::Write;

/// Lines of context around each change, as in `diff -u`.
const CONTEXT: usize = 3;
/// Largest gap (lines × lines) diffed exactly with an LCS table.
const MAX_TABLE: usize = 4_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Equal,
    Delete,
    Insert,
}

/// Returns a unified diff from `old` to `new`, or an empty string when they
/// are equal. Lines keep their own endings; a last line without one is
/// followed by `\ No newline at end of file`, so `patch -p0` applies it.
pub fn unified(old: &str, new: &str, old_name: &str, new_name: &str) -> String {
    if old == new {
        return String::new();
    }
    let a: Vec<&str> = old.split_inclusive('\n').collect();
    let b: Vec<&str> = new.split_inclusive('\n').collect();
    let mut ops = Vec::with_capacity(a.len().max(b.len()));
    diff_range(&a, &b, 0, a.len(), 0, b.len(), &mut ops);

    let mut out = format!("--- {old_name}\t(original)\n+++ {new_name}\t(formatted)\n");
    // Walk the ops, emitting one hunk per run of changes with context.
    let changes: Vec<usize> = (0..ops.len()).filter(|&k| ops[k] != Op::Equal).collect();
    // Line index on each side before op k.
    let mut pos = Vec::with_capacity(ops.len() + 1);
    let (mut i, mut j) = (0, 0);
    for op in &ops {
        pos.push((i, j));
        match op {
            Op::Equal => (i, j) = (i + 1, j + 1),
            Op::Delete => i += 1,
            Op::Insert => j += 1,
        }
    }
    pos.push((i, j));

    let mut c = 0;
    while c < changes.len() {
        let start = changes[c].saturating_sub(CONTEXT);
        let mut last = changes[c];
        while c + 1 < changes.len() && changes[c + 1] - last - 1 <= 2 * CONTEXT {
            c += 1;
            last = changes[c];
        }
        c += 1;
        let end = (last + 1 + CONTEXT).min(ops.len());
        let (i0, j0) = pos[start];
        let (i1, j1) = pos[end];
        let range = |s: usize, n: usize| match n {
            0 => format!("{s},0"),
            1 => format!("{}", s + 1),
            _ => format!("{},{n}", s + 1),
        };
        let _ = writeln!(out, "@@ -{} +{} @@", range(i0, i1 - i0), range(j0, j1 - j0));
        for k in start..end {
            let (i, j) = pos[k];
            let (sign, line) = match ops[k] {
                Op::Equal => (' ', a[i]),
                Op::Delete => ('-', a[i]),
                Op::Insert => ('+', b[j]),
            };
            out.push(sign);
            out.push_str(line);
            if !line.ends_with('\n') {
                out.push_str("\n\\ No newline at end of file\n");
            }
        }
    }
    out
}

/// Appends the ops that turn `a[a0..a1]` into `b[b0..b1]`.
fn diff_range(
    a: &[&str],
    b: &[&str],
    mut a0: usize,
    mut a1: usize,
    mut b0: usize,
    mut b1: usize,
    ops: &mut Vec<Op>,
) {
    // Common prefix and suffix.
    let mut prefix = 0;
    while a0 < a1 && b0 < b1 && a[a0] == b[b0] {
        a0 += 1;
        b0 += 1;
        prefix += 1;
    }
    let mut suffix = 0;
    while a0 < a1 && b0 < b1 && a[a1 - 1] == b[b1 - 1] {
        a1 -= 1;
        b1 -= 1;
        suffix += 1;
    }
    ops.extend(std::iter::repeat_n(Op::Equal, prefix));

    if a0 == a1 || b0 == b1 {
        ops.extend(std::iter::repeat_n(Op::Delete, a1 - a0));
        ops.extend(std::iter::repeat_n(Op::Insert, b1 - b0));
    } else {
        let anchors = unique_anchors(a, b, a0, a1, b0, b1);
        if anchors.is_empty() {
            lcs(a, b, a0, a1, b0, b1, ops);
        } else {
            let (mut i, mut j) = (a0, b0);
            for (ai, bj) in anchors {
                diff_range(a, b, i, ai, j, bj, ops);
                ops.push(Op::Equal);
                (i, j) = (ai + 1, bj + 1);
            }
            diff_range(a, b, i, a1, j, b1, ops);
        }
    }
    ops.extend(std::iter::repeat_n(Op::Equal, suffix));
}

/// Lines that occur exactly once in each range, as (a index, b index) pairs
/// forming the longest chain increasing on both sides (patience diff).
fn unique_anchors(
    a: &[&str],
    b: &[&str],
    a0: usize,
    a1: usize,
    b0: usize,
    b1: usize,
) -> Vec<(usize, usize)> {
    // line → (count in a, index in a, count in b, index in b)
    let mut seen: HashMap<&str, (u32, usize, u32, usize)> = HashMap::new();
    for (i, line) in a.iter().enumerate().take(a1).skip(a0) {
        let e = seen.entry(line).or_insert((0, i, 0, 0));
        e.0 += 1;
    }
    for (j, line) in b.iter().enumerate().take(b1).skip(b0) {
        if let Some(e) = seen.get_mut(line) {
            e.2 += 1;
            e.3 = j;
        }
    }
    let mut pairs: Vec<(usize, usize)> = seen
        .values()
        .filter(|e| e.0 == 1 && e.2 == 1)
        .map(|e| (e.1, e.3))
        .collect();
    pairs.sort_unstable();

    // Longest increasing subsequence on the b index (patience sorting).
    let mut tails: Vec<usize> = Vec::new(); // index into pairs
    let mut prev = vec![usize::MAX; pairs.len()];
    for k in 0..pairs.len() {
        let pile = tails.partition_point(|&t| pairs[t].1 < pairs[k].1);
        if pile > 0 {
            prev[k] = tails[pile - 1];
        }
        if pile == tails.len() {
            tails.push(k);
        } else {
            tails[pile] = k;
        }
    }
    let mut chain = Vec::with_capacity(tails.len());
    let mut k = tails.last().copied().unwrap_or(usize::MAX);
    while k != usize::MAX {
        chain.push(pairs[k]);
        k = prev[k];
    }
    chain.reverse();
    chain
}

/// Exact LCS diff of a gap, or delete-all/insert-all when the table would
/// be too large.
fn lcs(a: &[&str], b: &[&str], a0: usize, a1: usize, b0: usize, b1: usize, ops: &mut Vec<Op>) {
    let (n, m) = (a1 - a0, b1 - b0);
    if n.saturating_mul(m) > MAX_TABLE {
        ops.extend(std::iter::repeat_n(Op::Delete, n));
        ops.extend(std::iter::repeat_n(Op::Insert, m));
        return;
    }
    // t[i][j] = LCS length of a[a0+i..a1] and b[b0+j..b1].
    let w = m + 1;
    let mut t = vec![0u32; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i * w + j] = if a[a0 + i] == b[b0 + j] {
                t[(i + 1) * w + j + 1] + 1
            } else {
                t[(i + 1) * w + j].max(t[i * w + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[a0 + i] == b[b0 + j] {
            ops.push(Op::Equal);
            i += 1;
            j += 1;
        } else if t[(i + 1) * w + j] >= t[i * w + j + 1] {
            ops.push(Op::Delete);
            i += 1;
        } else {
            ops.push(Op::Insert);
            j += 1;
        }
    }
    ops.extend(std::iter::repeat_n(Op::Delete, n - i));
    ops.extend(std::iter::repeat_n(Op::Insert, m - j));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(d: &str) -> &str {
        d.split_once("(formatted)\n").unwrap().1
    }

    #[test]
    fn equal_texts_give_nothing() {
        assert_eq!(unified("a\n", "a\n", "x", "x"), "");
    }

    #[test]
    fn header_and_single_change() {
        let d = unified("a\nb\nc\n", "a\nB\nc\n", "f.asm", "f.asm");
        assert_eq!(
            d,
            "--- f.asm\t(original)\n+++ f.asm\t(formatted)\n\
             @@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n"
        );
    }

    #[test]
    fn hunks_split_and_merge_by_context() {
        let text = |changed: &[usize]| -> String {
            (1..=20)
                .map(|i| match changed.contains(&i) {
                    true => format!("new{i}\n"),
                    false => format!("{i}\n"),
                })
                .collect()
        };
        let hunks = |changed: &[usize]| -> Vec<String> {
            unified(&text(&[]), &text(changed), "f", "f")
                .lines()
                .filter(|l| l.starts_with("@@"))
                .map(String::from)
                .collect()
        };
        // Seven unchanged lines between changes: two hunks.
        assert_eq!(hunks(&[2, 10]), ["@@ -1,5 +1,5 @@", "@@ -7,7 +7,7 @@"]);
        // Six: one hunk, as with `diff -u`.
        assert_eq!(hunks(&[2, 9]), ["@@ -1,12 +1,12 @@"]);
        assert_eq!(hunks(&[20]), ["@@ -17,4 +17,4 @@"]);
    }

    #[test]
    fn insertions_deletions_and_empty_sides() {
        assert_eq!(
            body(&unified("a\n", "a\nb\n", "f", "f")),
            "@@ -1 +1,2 @@\n a\n+b\n"
        );
        assert_eq!(
            body(&unified("a\nb\n", "b\n", "f", "f")),
            "@@ -1,2 +1 @@\n-a\n b\n"
        );
        assert_eq!(body(&unified("", "a\n", "f", "f")), "@@ -0,0 +1 @@\n+a\n");
        assert_eq!(body(&unified("a\n", "", "f", "f")), "@@ -1 +0,0 @@\n-a\n");
    }

    #[test]
    fn missing_final_newline_is_marked() {
        assert_eq!(
            body(&unified("a", "a\n", "f", "f")),
            "@@ -1 +1 @@\n-a\n\\ No newline at end of file\n+a\n"
        );
    }

    /// Applies a diff's ops back to `old` and checks it gives `new`.
    fn roundtrip(old: &str, new: &str) {
        let a: Vec<&str> = old.split_inclusive('\n').collect();
        let b: Vec<&str> = new.split_inclusive('\n').collect();
        let mut ops = Vec::new();
        diff_range(&a, &b, 0, a.len(), 0, b.len(), &mut ops);
        let (mut i, mut j) = (0, 0);
        let mut out = String::new();
        for op in ops {
            match op {
                Op::Equal => {
                    assert_eq!(a[i], b[j]);
                    out.push_str(a[i]);
                    i += 1;
                    j += 1;
                }
                Op::Delete => i += 1,
                Op::Insert => {
                    out.push_str(b[j]);
                    j += 1;
                }
            }
        }
        assert_eq!((i, j), (a.len(), b.len()));
        assert_eq!(out, new);
    }

    #[test]
    fn ops_reconstruct_the_new_text() {
        roundtrip("a\nb\nc\nd\n", "d\nc\nb\na\n");
        roundtrip("x\na\nx\nb\nx\n", "a\nx\nx\nb\n");
        roundtrip("mov\n\nadd\n\n\nret", "  mov\n\n  add\n\n  ret\n");
        // A gap above MAX_TABLE falls back to a block replacement.
        let old: String = (0..2100).map(|i| format!("{}\n", i % 7)).collect();
        let new: String = (0..2100).map(|i| format!("{}\n", i % 5)).collect();
        roundtrip(&old, &new);
    }

    #[test]
    fn lcs_is_minimal_on_small_gaps() {
        let d = unified("a\nx\nb\nx\nc\n", "a\nb\nx\nc\n", "f", "f");
        assert_eq!(
            d.lines().filter(|l| l.starts_with(['-', '+'])).count(),
            2 + 1
        );
    }
}
