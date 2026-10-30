use proptest::prelude::*;

use super::*;
use crate::tests::view_with;

/// Characters the matcher treats specially: both cases, a separator, digits
/// and punctuation, with a few that need more than one UTF-16 unit or byte.
fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            4 => prop::sample::select(vec!['a', 'b', 'c', 'd']),
            3 => prop::sample::select(vec!['A', 'B', 'C', 'D']),
            2 => Just(' '),
            1 => prop::sample::select(vec!['-', '1', '_', 'é', '🦀']),
        ],
        0..14,
    )
    .prop_map(|cs| cs.into_iter().collect())
}

fn is_subsequence(needle: &[char], haystack: &[char]) -> bool {
    let mut rest = haystack.iter();
    needle.iter().all(|n| rest.any(|h| h == n))
}

fn lower(s: &str) -> Vec<char> {
    s.chars().map(fold).collect()
}

/// What `ranges` pick out of `name`.
fn picked(name: &str, ranges: &[(usize, usize)]) -> String {
    let units: Vec<u16> = name.encode_utf16().collect();
    ranges
        .iter()
        .map(|&(s, e)| String::from_utf16(&units[s..e]).unwrap())
        .collect()
}

fn titles(hits: &[TitleHit]) -> Vec<&str> {
    hits.iter().map(|h| h.entity.title.as_str()).collect()
}

proptest! {
    #[test]
    fn a_name_matches_itself(name in text()) {
        prop_assume!(!name.trim().is_empty());
        prop_assert!(fuzzy_match(&name, &name).is_some());
    }

    /// Without capitals or spaces the query is just a subsequence, ignoring case.
    #[test]
    fn lower_case_queries_match_subsequences(
        query in "[a-d1é-]{0,5}", name in text(),
    ) {
        let expected = !query.is_empty()
            && is_subsequence(&lower(&query), &lower(&name));
        prop_assert_eq!(fuzzy_match(&query, &name).is_some(), expected);
    }

    /// A match says where: ascending, separate runs in the name that spell the
    /// query, whatever the name is made of.
    #[test]
    fn ranges_pick_out_the_query(query in text(), name in text()) {
        let Some(m) = fuzzy_match(&query, &name) else { return Ok(()); };
        let len = name.encode_utf16().count();
        let mut end = 0;
        for &(s, e) in &m.ranges {
            prop_assert!(end < s || (end == 0 && s == 0), "{:?}", m.ranges);
            prop_assert!(s < e && e <= len);
            end = e;
        }
        let typed: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).collect();
        let got = picked(&name, &m.ranges);
        prop_assert_eq!(lower(&got), lower(&typed.iter().collect::<String>()));
    }

    /// Capitals are the initials of words: typing them finds exactly those.
    #[test]
    fn capitals_match_word_starts(
        words in prop::collection::vec(("[a-z]{1,4}", any::<bool>()), 1..5),
    ) {
        let name = words
            .iter()
            .map(|(w, cap)| if *cap { w[..1].to_uppercase() + &w[1..] } else { w.clone() })
            .collect::<Vec<_>>()
            .join(" ");
        let query: String = words.iter().map(|(w, _)| w[..1].to_uppercase()).collect();
        let m = fuzzy_match(&query, &name).expect("initials match");
        let mut at = 0;
        let starts: Vec<usize> = words
            .iter()
            .map(|(w, _)| { let s = at; at += w.len() + 1; s })
            .collect();
        let got: Vec<usize> = m.ranges.iter().flat_map(|&(s, e)| s..e).collect();
        prop_assert_eq!(got, starts);
    }

    /// The same letters typed together beat them spread out.
    #[test]
    fn an_exact_match_beats_a_longer_name(name in "[a-d]{2,5}", extra in "[a-d]{1,3}") {
        let longer = format!("{name}{extra}");
        let exact = fuzzy_match(&name, &name).unwrap().score;
        prop_assert!(exact > fuzzy_match(&name, &longer).unwrap().score);
    }
}

#[test]
fn capitals_must_start_words() {
    assert!(fuzzy_match("QD", "Quiddity Design").is_some());
    assert!(fuzzy_match("QD", "Quiddity").is_none());
    assert!(fuzzy_match("qd", "Quiddity").is_some());
    // The same capital counts wherever it is, so an acronym finds itself.
    assert!(fuzzy_match("HTTP", "HTTP").is_some());
    assert!(fuzzy_match("HS", "HTTPServer").is_some());
    assert!(fuzzy_match("CA", "camelCaseData").is_none());
    assert!(fuzzy_match("CC", "camelCaseData").is_some());
}

#[test]
fn a_space_starts_a_word() {
    assert!(fuzzy_match("quid des", "Quiddity Design").is_some());
    assert!(fuzzy_match("quid ign", "Quiddity Design").is_none());
    assert!(fuzzy_match("  quid ", "Quiddity").is_some());
    assert!(fuzzy_match("   ", "Quiddity").is_none());
    assert!(fuzzy_match("", "Quiddity").is_none());
}

#[test]
fn ranges_are_in_utf16_units() {
    let m = fuzzy_match("crab", "🦀 Crab").unwrap();
    assert_eq!(m.ranges, vec![(3, 7)]);
    let m = fuzzy_match("ab", "a🦀b").unwrap();
    assert_eq!(m.ranges, vec![(0, 1), (3, 4)]);
}

#[test]
fn the_best_alignment_wins() {
    // The second "d" begins a word; the first doesn't.
    let m = fuzzy_match("qd", "Quiddity Design").unwrap();
    assert_eq!(m.ranges, vec![(0, 1), (9, 10)]);
    // Typed together beats scattered.
    let m = fuzzy_match("abc", "xaxbabc").unwrap();
    assert_eq!(m.ranges, vec![(4, 7)]);
}

#[test]
fn titles_rank_exact_then_prefix_then_word_then_scattered() {
    let v = view_with(|g| {
        for name in ["Reading Loro list", "Loro CRDT", "Loro", "Colour room"] {
            g.create_entity(name, &[]).unwrap();
        }
    });
    let hits = v.search_titles("loro", 10).unwrap();
    assert_eq!(
        titles(&hits),
        ["Loro", "Loro CRDT", "Reading Loro list", "Colour room"]
    );
    assert_eq!(hits[0].ranges, vec![(0, 4)]);
}

#[test]
fn titles_are_limited_and_ties_keep_title_order() {
    let v = view_with(|g| {
        for name in ["Bb", "Ba", "Bc"] {
            g.create_entity(name, &[]).unwrap();
        }
    });
    assert_eq!(
        titles(&v.search_titles("b", 10).unwrap()),
        ["Ba", "Bb", "Bc"]
    );
    assert_eq!(titles(&v.search_titles("b", 2).unwrap()), ["Ba", "Bb"]);
    assert!(v.search_titles("zzz", 10).unwrap().is_empty());
}

#[test]
fn word_starts_are_found_in_every_style() {
    let starts = |s: &str| -> String {
        let chars: Vec<char> = s.chars().collect();
        let marks = word_starts(&chars);
        chars
            .iter()
            .zip(marks)
            .map(|(&c, m)| if m { c } else { '.' })
            .collect()
    };
    assert_eq!(starts("quiddity design"), "q........d.....");
    assert_eq!(starts("camelCase"), "c....C...");
    assert_eq!(starts("HTTPServer"), "H...S.....");
    assert_eq!(starts("ABC"), "A..");
    assert_eq!(starts("a1 b-2"), "a..b.2");
    assert_eq!(starts("-x"), ".x");
}

/// The scores themselves, built from the weights, so that each weight is
/// checked to do what it says without pinning the numbers.
#[test]
fn scores_add_up_from_their_parts() {
    let score = |q, n| fuzzy_match(q, n).unwrap().score;
    // One character: a match, at a word start, in the right case, at the start.
    assert_eq!(score("a", "a"), MATCH + WORD_START + SAME_CASE + PREFIX - 1);
    assert_eq!(score("a", "A"), MATCH + WORD_START + PREFIX - 1);
    // Typed together, then with a gap, then in the middle of a word.
    assert_eq!(
        score("ab", "ab"),
        2 * (MATCH + SAME_CASE) + WORD_START + PREFIX + ADJACENT - 2
    );
    assert_eq!(
        score("ab", "axb"),
        2 * (MATCH + SAME_CASE) + WORD_START + PREFIX - GAP - 3
    );
    assert_eq!(score("b", "ab"), MATCH + SAME_CASE - 2);
    assert_eq!(score("b", "a b"), MATCH + WORD_START + SAME_CASE - 3);
}
