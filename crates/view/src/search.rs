//! Search: fuzzy matching of names, and the title search built on it.

use serde::Serialize;

use crate::query::EntityRef;
use crate::{Result, View};

/// What a matched character is worth, before bonuses.
const MATCH: i32 = 10;
/// For matching the first character of a word.
const WORD_START: i32 = 15;
/// For matching straight after the previous character, so that a run typed
/// together beats the same letters scattered.
const ADJACENT: i32 = 20;
/// For starting at the first character of the name.
const PREFIX: i32 = 10;
/// For each stretch of skipped characters between two matches.
const GAP: i32 = 8;
/// For matching the case that was typed, which settles ties.
const SAME_CASE: i32 = 1;

/// Where a query matched a name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Match {
    /// Higher is better, and only means anything beside the scores of other
    /// names for the same query.
    pub score: i32,
    /// The matched text as `[start, end)` runs, in UTF-16 units of the name
    /// and ascending, for highlighting.
    pub ranges: Vec<(usize, usize)>,
}

struct QueryChar {
    /// What it matches, ignoring case.
    folded: char,
    /// As typed.
    typed: char,
    /// Must match the first character of a word (or, for a capital, the same
    /// capital): typed as a capital, or a letter or digit straight after a space.
    word_start: bool,
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// The characters that must be matched: whitespace separates and is never
/// matched itself, but makes the next character start a word.
fn query_chars(query: &str) -> Vec<QueryChar> {
    let mut out = Vec::new();
    let mut after_space = false;
    for c in query.chars() {
        if c.is_whitespace() {
            after_space = true;
            continue;
        }
        out.push(QueryChar {
            folded: fold(c),
            typed: c,
            word_start: c.is_uppercase() || (after_space && c.is_alphanumeric() && !out.is_empty()),
        });
        after_space = false;
    }
    out
}

/// Whether each character begins a word: the first letter or digit after
/// anything else, a capital after a lower-case letter (`camelCase`), and the
/// last capital of a run that a lower-case letter follows (`HTTPServer`).
fn word_starts(chars: &[char]) -> Vec<bool> {
    (0..chars.len())
        .map(|i| {
            let c = chars[i];
            let Some(&prev) = i.checked_sub(1).map(|p| &chars[p]) else {
                return c.is_alphanumeric();
            };
            c.is_alphanumeric()
                && (!prev.is_alphanumeric()
                    || (c.is_uppercase() && prev.is_lowercase())
                    || (c.is_uppercase()
                        && prev.is_uppercase()
                        && chars.get(i + 1).is_some_and(|n| n.is_lowercase())))
        })
        .collect()
}

/// Matches `query` against `name` the way an IDE's "go to" does: the query's
/// characters must appear in order, ignoring case, except that
///
/// - a capital must be the first letter of a word (or the same capital), so
///   `QD` finds `Quiddity Design`, and
/// - a character after a space must also start a word, so `quid d` doesn't
///   find `Quiddity`.
///
/// Of the ways to line the query up, the best scoring is chosen: runs typed
/// together, word starts and the start of the name all count for more.
/// `None` if it doesn't match, or the query is only whitespace.
#[must_use]
pub fn fuzzy_match(query: &str, name: &str) -> Option<Match> {
    let query = query_chars(query);
    let chars: Vec<char> = name.chars().collect();
    let starts = word_starts(&chars);
    let folded: Vec<char> = chars.iter().map(|&c| fold(c)).collect();
    let (m, n) = (query.len(), chars.len());
    if m == 0 || m > n {
        return None;
    }

    let fits = |i: usize, j: usize| {
        let q = &query[i];
        q.folded == folded[j]
            && (!q.word_start || starts[j] || (q.typed.is_uppercase() && chars[j] == q.typed))
    };
    let here = |i: usize, j: usize| {
        MATCH
            + if starts[j] { WORD_START } else { 0 }
            + if chars[j] == query[i].typed {
                SAME_CASE
            } else {
                0
            }
    };

    // best[i][j]: the best score with query[i] matched at chars[j]; from[i][j]
    // is where query[i - 1] was in that alignment.
    let mut best = vec![vec![None::<i32>; n]; m];
    let mut from = vec![vec![0usize; n]; m];
    for j in (0..n).filter(|&j| fits(0, j)) {
        best[0][j] = Some(here(0, j) + if j == 0 { PREFIX } else { 0 });
    }
    for i in 1..m {
        // The best earlier match that leaves a gap before j, as j advances.
        let mut gapped: Option<(i32, usize)> = None;
        for j in i..n {
            if j >= 2 {
                let earlier = best[i - 1][j - 2].map(|s| (s, j - 2));
                gapped = gapped.into_iter().chain(earlier).max_by_key(|&(s, _)| s);
            }
            if !fits(i, j) {
                continue;
            }
            let adjacent = best[i - 1][j - 1].map(|s| (s + ADJACENT, j - 1));
            let gapped = gapped.map(|(s, k)| (s - GAP, k));
            let Some((score, k)) = [adjacent, gapped]
                .into_iter()
                .flatten()
                .max_by_key(|&(s, _)| s)
            else {
                continue;
            };
            best[i][j] = Some(score + here(i, j));
            from[i][j] = k;
        }
    }

    // The earliest of the equally good ends, so the answer doesn't depend on
    // iteration order.
    let (end, score) = (0..n).filter_map(|j| best[m - 1][j].map(|s| (j, s))).fold(
        None,
        |top: Option<(usize, i32)>, (j, s)| match top {
            Some((_, t)) if t >= s => top,
            _ => Some((j, s)),
        },
    )?;

    let mut at = vec![0; m];
    at[m - 1] = end;
    for i in (1..m).rev() {
        at[i - 1] = from[i][at[i]];
    }
    Some(Match {
        // Shorter names beat longer ones that match the same way.
        score: score - n as i32,
        ranges: utf16_runs(&chars, &at),
    })
}

/// `at` (ascending character indices of `chars`) as runs of UTF-16 units.
fn utf16_runs(chars: &[char], at: &[usize]) -> Vec<(usize, usize)> {
    let mut offsets = Vec::with_capacity(chars.len() + 1);
    let mut units = 0;
    for c in chars {
        offsets.push(units);
        units += c.len_utf16();
    }
    offsets.push(units);

    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (k, &j) in at.iter().enumerate() {
        match runs.last_mut() {
            Some(run) if at[k - 1] + 1 == j => run.1 = offsets[j + 1],
            _ => runs.push((offsets[j], offsets[j + 1])),
        }
    }
    runs
}

/// An entity whose title matched a search.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TitleHit {
    pub entity: EntityRef,
    pub score: i32,
    /// As [`Match::ranges`], in the title.
    pub ranges: Vec<(usize, usize)>,
}

impl View {
    /// The entities whose titles match `query` (see [`fuzzy_match`]), best
    /// first and at most `limit`. Equal scores are in title order.
    pub fn search_titles(&self, query: &str, limit: usize) -> Result<Vec<TitleHit>> {
        let mut hits: Vec<TitleHit> = self
            .entities()?
            .into_iter()
            .filter_map(|entity| {
                let m = fuzzy_match(query, &entity.title)?;
                Some(TitleHit {
                    entity,
                    score: m.score,
                    ranges: m.ranges,
                })
            })
            .collect();
        // Stable, and `entities` is in title order.
        hits.sort_by_key(|h| std::cmp::Reverse(h.score));
        hits.truncate(limit);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests;
