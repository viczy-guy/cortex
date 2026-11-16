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
