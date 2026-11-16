//! Line-level diff stats (+N −M) for file edits shown in the island ticker.
//!
//! This is intentionally a *stat*, not a rendered diff: a bounded LCS gives
//! exact numbers for normal edits, and a cheap fallback keeps worst-case cost
//! flat for very large inputs.

/// Above this many lines per side we skip the LCS and use the cheap bound.
const EXACT_LIMIT: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct DiffStat {
    pub added: usize,
    pub removed: usize,
}

/// Diff stats between the old and new content of one region or file.
pub fn stat(old: &str, new: &str) -> DiffStat {
    let old_lines: Vec<&str> = non_empty_lines(old);
    let new_lines: Vec<&str> = non_empty_lines(new);

    if old_lines.len() > EXACT_LIMIT || new_lines.len() > EXACT_LIMIT {
        // Cheap bound: every line counts, minus what trivially matches at the ends.
        let common = common_affix(&old_lines, &new_lines);
        return DiffStat {
            added: new_lines.len() - common,
            removed: old_lines.len() - common,
        };
    }

    let common = lcs_len(&old_lines, &new_lines);
    DiffStat {
        added: new_lines.len() - common,
        removed: old_lines.len() - common,
    }
}

fn non_empty_lines(s: &str) -> Vec<&str> {
    if s.is_empty() {
        Vec::new()
    } else {
        s.lines().collect()
    }
}

/// Length of the longest common subsequence of lines (classic DP, O(n·m) bounded
/// by `EXACT_LIMIT`²).
fn lcs_len(a: &[&str], b: &[&str]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let mut prev = vec![0usize; b.len() + 1];
    let mut cur = vec![0usize; b.len() + 1];
    for line_a in a {
        for (j, line_b) in b.iter().enumerate() {
            cur[j + 1] = if line_a == line_b {
                prev[j] + 1
            } else {
                prev[j + 1].max(cur[j])
            };
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// One line of a rendered diff.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DiffLine {
    pub kind: LineKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Same,
    Added,
    Removed,
}

/// Cells of the LCS table above which [`lines`] stops being exact.
const RENDER_CELL_LIMIT: usize = 4_000_000;

/// A full line-by-line diff, for showing the user exactly what will change
/// (used by the hook installer). Exact for normal files; for huge inputs it
/// degrades to "everything removed, everything added", which is still true.
pub fn lines(old: &str, new: &str) -> Vec<DiffLine> {
    let a = non_empty_lines(old);
    let b = non_empty_lines(new);
    let line = |kind, text: &str| DiffLine {
        kind,
        text: text.to_owned(),
    };

    if a.len().saturating_mul(b.len()) > RENDER_CELL_LIMIT {
        return a
            .iter()
            .map(|t| line(LineKind::Removed, t))
            .chain(b.iter().map(|t| line(LineKind::Added, t)))
            .collect();
    }

    // table[i][j] = LCS length of a[i..] and b[j..].
    let mut table = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            table[i][j] = if a[i] == b[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }

    let mut out = Vec::with_capacity(a.len().max(b.len()));
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push(line(LineKind::Same, a[i]));
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            out.push(line(LineKind::Removed, a[i]));
            i += 1;
        } else {
            out.push(line(LineKind::Added, b[j]));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|t| line(LineKind::Removed, t)));
    out.extend(b[j..].iter().map(|t| line(LineKind::Added, t)));
    out
}

/// Matching lines at the start plus at the end (without overlap).
fn common_affix(a: &[&str], b: &[&str]) -> usize {
    let max = a.len().min(b.len());
    let prefix = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(max - prefix)
        .take_while(|(x, y)| x == y)
        .count();
    prefix + suffix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_diff_marks_each_line() {
        let kinds: Vec<(LineKind, String)> = lines("a\nb\nc", "a\nx\nc\nd")
            .into_iter()
            .map(|l| (l.kind, l.text))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (LineKind::Same, "a".to_string()),
                (LineKind::Removed, "b".to_string()),
                (LineKind::Added, "x".to_string()),
                (LineKind::Same, "c".to_string()),
                (LineKind::Added, "d".to_string()),
            ]
        );
    }

    #[test]
    fn rendered_diff_of_identical_text_has_no_changes() {
        assert!(lines("a\nb", "a\nb")
            .iter()
            .all(|l| l.kind == LineKind::Same));
    }

    #[test]
    fn identical_is_zero() {
        assert_eq!(
            stat("a\nb\nc", "a\nb\nc"),
            DiffStat {
                added: 0,
                removed: 0
            }
        );
    }

    #[test]
    fn pure_addition() {
        assert_eq!(
            stat("", "a\nb"),
            DiffStat {
                added: 2,
                removed: 0
            }
        );
    }

    #[test]
    fn pure_removal() {
        assert_eq!(
            stat("a\nb", ""),
            DiffStat {
                added: 0,
                removed: 2
            }
        );
    }

    #[test]
    fn replacement_in_the_middle() {
        // b -> x : one added, one removed; a and c survive.
        assert_eq!(
            stat("a\nb\nc", "a\nx\nc"),
            DiffStat {
                added: 1,
                removed: 1
            }
        );
    }

    #[test]
    fn large_input_uses_the_bound_and_stays_sane() {
        let old: String = (0..1000).map(|i| format!("line {i}\n")).collect();
        let mut new = old.clone();
        new.push_str("extra\n");
        let s = stat(&old, &new);
        assert_eq!(s.removed, 0);
        assert_eq!(s.added, 1);
    }
}
