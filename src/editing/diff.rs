//! Preview: a unified diff of one file's exact original and proposed text,
//! for reading. It never supplies the data applying uses, which is the
//! batch's own bytes. Lines end at ECMAScript line breaks and compare with
//! their breaks, so a changed line break is a changed line; a line shows
//! without its break, and a header names breaks other than LF.

use crate::source::line_breaks;

/// Context lines around each change.
const CONTEXT: usize = 3;
/// The most line differences a minimal diff searches for, beyond what the
/// original and proposed text share at their start and end; with more, the
/// middle shows as one replacement, still exact, just not minimal. The
/// search keeps the diagonals of each step, about `2 × LIMIT²` integers at
/// the limit.
const MINIMAL_LIMIT: usize = 2_000;

/// One line of a text: its content and its break, empty for the last line.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Line<'t> {
    content: &'t str,
    end: &'t str,
}

fn lines(text: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for line_break in line_breaks(text) {
        lines.push(Line {
            content: &text[start..line_break.start],
            end: &text[line_break.clone()],
        });
        start = line_break.end;
    }
    if start < text.len() {
        lines.push(Line {
            content: &text[start..],
            end: "",
        });
    }
    lines
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    Equal,
    Delete,
    Insert,
}

/// The edit script from `a` to `b`: Myers' shortest script over the lines
/// between their common prefix and suffix, or a plain replacement of that
/// middle when it has more than `MINIMAL_LIMIT` differences.
fn script(a: &[Line<'_>], b: &[Line<'_>]) -> Vec<Op> {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    let mut ops = vec![Op::Equal; prefix];
    match myers(am, bm, MINIMAL_LIMIT) {
        Some(middle) => ops.extend(middle),
        None => {
            ops.extend(std::iter::repeat_n(Op::Delete, am.len()));
            ops.extend(std::iter::repeat_n(Op::Insert, bm.len()));
        }
    }
    ops.extend(std::iter::repeat_n(Op::Equal, suffix));
    ops
}

/// Myers' O((N+M)D) shortest edit script, backtracked from the diagonals
/// each step reached, or `None` when it needs more than `limit` differences.
fn myers(a: &[Line<'_>], b: &[Line<'_>], limit: usize) -> Option<Vec<Op>> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    // The furthest x on each diagonal k, at index k + d, for each step d.
    let mut steps: Vec<Vec<isize>> = Vec::new();
    let reached = |steps: &[Vec<isize>], d: isize, k: isize| -> isize {
        if d < 0 || k < -d || k > d {
            return -1;
        }
        steps[d as usize][(k + d) as usize]
    };
    let mut found = None;
    for d in 0..=(limit.min((n + m) as usize) as isize) {
        let mut v = vec![0isize; (2 * d + 1) as usize];
        let mut k = -d;
        while k <= d {
            let mut x = if d == 0 {
                0
            } else if k == -d
                || k != d && reached(&steps, d - 1, k - 1) < reached(&steps, d - 1, k + 1)
            {
                reached(&steps, d - 1, k + 1)
            } else {
                reached(&steps, d - 1, k - 1) + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[(k + d) as usize] = x;
            if x >= n && y >= m && found.is_none() {
                found = Some(d);
            }
            k += 2;
        }
        steps.push(v);
        if found.is_some() {
            break;
        }
    }
    let mut d = found?;
    let mut ops = Vec::new();
    let (mut x, mut y) = (n, m);
    while d > 0 {
        let k = x - y;
        let previous =
            if k == -d || k != d && reached(&steps, d - 1, k - 1) < reached(&steps, d - 1, k + 1) {
                k + 1
            } else {
                k - 1
            };
        let previous_x = reached(&steps, d - 1, previous);
        let previous_y = previous_x - previous;
        while x > previous_x && y > previous_y {
            ops.push(Op::Equal);
            x -= 1;
            y -= 1;
        }
        ops.push(if x == previous_x {
            Op::Insert
        } else {
            Op::Delete
        });
        x = previous_x;
        y = previous_y;
        d -= 1;
    }
    while x > 0 && y > 0 {
        ops.push(Op::Equal);
        x -= 1;
        y -= 1;
    }
    ops.reverse();
    Some(ops)
}

/// The breaks of `lines` other than LF, named for a header.
fn breaks(lines: &[Line<'_>]) -> Option<String> {
    let mut names: Vec<&str> = Vec::new();
    for line in lines {
        let name = match line.end {
            "\r\n" => "CRLF",
            "\r" => "CR",
            "\u{2028}" => "U+2028",
            "\u{2029}" => "U+2029",
            _ => continue,
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }
    (!names.is_empty()).then(|| names.join(", "))
}

/// A unified diff from `original` to `proposed` of the file named `label`,
/// empty when they are equal.
pub(super) fn unified(label: &str, original: &str, proposed: &str) -> String {
    if original == proposed {
        return String::new();
    }
    let (a, b) = (lines(original), lines(proposed));
    let ops = script(&a, &b);
    let mut text = format!("--- {label}\n+++ {label}\n");
    if let Some(breaks) = breaks(&a).or_else(|| breaks(&b)) {
        text.push_str(&format!("# line breaks: {breaks}\n"));
    }
    // Positions in `ops`, `a`, and `b` where each op begins.
    let mut at = Vec::with_capacity(ops.len());
    let (mut i, mut j) = (0, 0);
    for op in &ops {
        at.push((i, j));
        match op {
            Op::Equal => {
                i += 1;
                j += 1;
            }
            Op::Delete => i += 1,
            Op::Insert => j += 1,
        }
    }
    let changed: Vec<usize> = (0..ops.len()).filter(|&k| ops[k] != Op::Equal).collect();
    let mut k = 0;
    while k < changed.len() {
        let start = changed[k].saturating_sub(CONTEXT);
        let mut end = changed[k] + 1;
        while k + 1 < changed.len() && changed[k + 1] <= end + 2 * CONTEXT {
            k += 1;
            end = changed[k] + 1;
        }
        let end = (end + CONTEXT).min(ops.len());
        let (a_start, b_start) = at[start];
        let a_count = ops[start..end]
            .iter()
            .filter(|op| **op != Op::Insert)
            .count();
        let b_count = ops[start..end]
            .iter()
            .filter(|op| **op != Op::Delete)
            .count();
        let range = |first: usize, count: usize| match count {
            0 => format!("{first},0"),
            1 => format!("{}", first + 1),
            _ => format!("{},{count}", first + 1),
        };
        text.push_str(&format!(
            "@@ -{} +{} @@\n",
            range(a_start, a_count),
            range(b_start, b_count)
        ));
        for index in start..end {
            let (ai, bj) = at[index];
            let (sign, line) = match ops[index] {
                Op::Equal => (' ', a[ai]),
                Op::Delete => ('-', a[ai]),
                Op::Insert => ('+', b[bj]),
            };
            text.push(sign);
            text.push_str(line.content);
            text.push('\n');
            if line.end.is_empty() {
                text.push_str("\\ No newline at end of file\n");
            }
        }
        k += 1;
    }
    text
}

#[cfg(test)]
mod tests {
    use super::unified;

    #[test]
    fn hunks_show_context_and_merge_nearby_changes() {
        let original = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";
        let proposed = "a\nb\nC\nd\ne\nf\ng\nh\nI\nj\nk\n";
        assert_eq!(
            unified("x.ts", original, proposed),
            "--- x.ts\n+++ x.ts\n@@ -1,10 +1,11 @@\n a\n b\n-c\n+C\n d\n e\n f\n g\n h\n-i\n+I\n j\n+k\n"
        );
        assert_eq!(unified("x.ts", "a\n", "a\n"), "");
    }

    #[test]
    fn line_breaks_and_a_missing_final_newline_are_visible() {
        assert_eq!(
            unified("x.ts", "a\r\nb", "a\r\nb\r\nc"),
            "--- x.ts\n+++ x.ts\n# line breaks: CRLF\n@@ -1,2 +1,3 @@\n a\n-b\n\\ No newline at end of file\n+b\n+c\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn a_large_rewrite_is_one_exact_replacement_within_bounded_memory() {
        // 6,000 lines replaced by 6,000 others: more differences than a
        // minimal script searches, so the middle shows as one replacement.
        let original: String = (0..6_000).map(|line| format!("old {line}\n")).collect();
        let proposed: String = (0..6_000).map(|line| format!("new {line}\n")).collect();
        let started = std::time::Instant::now();
        let diff = unified("x.ts", &original, &proposed);
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        assert!(diff.starts_with("--- x.ts\n+++ x.ts\n@@ -1,6000 +1,6000 @@\n-old 0\n"));
        assert_eq!(
            diff.lines().filter(|line| line.starts_with('-')).count(),
            6_001
        );
        assert_eq!(
            diff.lines().filter(|line| line.starts_with('+')).count(),
            6_001
        );
    }

    #[test]
    fn an_insertion_into_an_empty_range_counts_from_zero() {
        assert_eq!(
            unified("x.ts", "", "a\n"),
            "--- x.ts\n+++ x.ts\n@@ -0,0 +1 @@\n+a\n"
        );
    }
}
