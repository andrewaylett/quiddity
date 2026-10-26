use jiff::ToSpan;
use jiff::civil::Date;
use quiddity_core::syntax::{Link, Value};
use quiddity_core::{BlockId, Graph};

use super::*;

fn text(s: &str) -> Option<Value> {
    Some(Value::Text(s.to_owned()))
}

fn today() -> Date {
    "2026-10-26".parse().unwrap()
}

/// A view on a graph that `build` has filled.
fn view_with(build: impl FnOnce(&Graph)) -> View {
    let graph = Graph::new();
    build(&graph);
    graph.commit();
    View::new(graph).unwrap()
}

fn note(graph: &Graph, parent: BlockId, text: &str) -> BlockId {
    graph.create_block(Some(parent), None, text).unwrap()
}

fn id_of(v: &View, title: &str) -> String {
    v.entities()
        .unwrap()
        .into_iter()
        .find(|e| e.title == title)
        .unwrap()
        .id
}

fn titles(refs: &[Reference]) -> Vec<&str> {
    refs.iter().map(|r| r.source.title.as_str()).collect()
}

fn texts(blocks: &[Block]) -> Vec<&str> {
    blocks.iter().map(|b| b.text.as_str()).collect()
}

/// An entity with the given top-level blocks, and the view on it.
fn page(blocks: &[&str]) -> (View, String, Vec<String>) {
    let mut ids = Vec::new();
    let v = view_with(|g| {
        let page = g.create_entity("Page", &[]).unwrap();
        ids = blocks
            .iter()
            .map(|t| note(g, page, t).to_string())
            .collect();
    });
    let page = id_of(&v, "Page");
    (v, page, ids)
}

fn top_level(v: &View, page: &str) -> Vec<String> {
    v.entity(page)
        .unwrap()
        .blocks
        .into_iter()
        .map(|b| b.text)
        .collect()
}

mod queries {
    use super::*;

    fn lots() -> View {
        view_with(|g| {
            g.create_entity("Zebra", &[]).unwrap();
            g.create_entity("apple", &[("favourite", text("true"))])
                .unwrap();
            g.create_entity("Mango", &[("favourite", text("no"))])
                .unwrap();
            let quiddity = g
                .create_entity("Quiddity", &[("favourite", text("Yes"))])
                .unwrap();
            let n = note(g, quiddity, "Built on [[apple]], [[Nope]]");
            note(g, n, "nested");
        })
    }

    #[test]
    fn lists_entities_by_name_ignoring_case() {
        let names: Vec<_> = lots()
            .entities()
            .unwrap()
            .into_iter()
            .map(|e| e.title)
            .collect();
        assert_eq!(names, ["apple", "Mango", "Quiddity", "Zebra"]);
    }

    #[test]
    fn favourites_come_from_the_property() {
        let names: Vec<_> = lots()
            .favourites()
            .unwrap()
            .into_iter()
            .map(|e| e.title)
            .collect();
        assert_eq!(names, ["apple", "Quiddity"]);
    }

    #[test]
    fn an_entity_has_nested_blocks_and_resolved_links() {
        let v = lots();
        let e = v.entity(&id_of(&v, "Quiddity")).unwrap();
        assert_eq!(e.title, "Quiddity");
        assert_eq!(e.blocks.len(), 2, "the properties block, then the note");
        assert_eq!(e.blocks[1].text, "Built on [[apple]], [[Nope]]");
        assert_eq!(e.blocks[1].children[0].text, "nested");
        let apple = v.entities().unwrap().remove(0).id;
        assert_eq!(e.links["apple"], LinkTarget::Entity { id: apple });
        assert_eq!(e.links["Nope"], LinkTarget::Missing);
    }

    #[test]
    fn only_entities_can_be_opened() {
        let v = lots();
        let id = v.entities().unwrap().remove(0).id;
        let block = v.entity(&id).unwrap().blocks[0].id.clone();
        assert!(v.entity(&block).is_err(), "a block is not an entity");
        assert!(v.entity("nonsense").is_err());
    }

    #[test]
    fn journals_run_newest_first_from_today() {
        let v = view_with(|g| {
            let index = quiddity_core::Index::build(g).unwrap();
            for date in [
                today().checked_sub(5.days()).unwrap(),
                today().checked_sub(1.day()).unwrap(),
                today().checked_add(1.day()).unwrap(),
            ] {
                quiddity_core::ensure_journal(g, &index, date).unwrap();
            }
            g.create_entity("Not a journal", &[]).unwrap();
        });
        let titles = |v: &View| -> Vec<String> {
            v.journals(today())
                .unwrap()
                .into_iter()
                .map(|e| e.title)
                .collect()
        };
        assert_eq!(
            titles(&v),
            ["2026-10-21", "2026-10-25"]
                .map(str::to_owned)
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
        );
        let made = v.ensure_journal(today()).unwrap();
        assert_eq!(made.title, "2026-10-26");
        assert_eq!(titles(&v), ["2026-10-26", "2026-10-25", "2026-10-21"]);
        assert_eq!(v.ensure_journal(today()).unwrap(), made, "created once");
    }

    #[test]
    fn references_are_grouped_by_entity_without_nesting_or_self_links() {
        let v = view_with(|g| {
            let alpha = g.create_entity("Alpha", &[]).unwrap();
            note(g, alpha, "talking to myself, [[Alpha]]");
            let beta = g.create_entity("Beta", &[]).unwrap();
            let outer = note(g, beta, "see [[Alpha]]");
            note(g, outer, "and again [[Alpha]]");
            note(g, beta, "unrelated");
            let to_alpha = Value::Link(Link {
                name: "Alpha".into(),
                qualifiers: Vec::new(),
            });
            let gamma = g
                .create_entity("Gamma", &[("type", Some(to_alpha))])
                .unwrap();
            note(g, gamma, "[[Alpha]] too, and [[Nope]]");
        });
        let refs = v.references(&id_of(&v, "Alpha")).unwrap();
        assert_eq!(titles(&refs), ["Beta", "Gamma"]);
        let beta = &refs[0];
        assert_eq!(
            beta.blocks.len(),
            1,
            "the inner block is shown inside the outer"
        );
        assert_eq!(beta.blocks[0].text, "see [[Alpha]]");
        assert_eq!(beta.blocks[0].children[0].text, "and again [[Alpha]]");
        let gamma = &refs[1];
        assert_eq!(gamma.blocks.len(), 2, "its property and its note");
        assert_eq!(gamma.links["Nope"], LinkTarget::Missing);
        assert!(matches!(gamma.links["Alpha"], LinkTarget::Entity { .. }));
    }

    #[test]
    fn references_list_journals_first_and_newest_first() {
        let v = view_with(|g| {
            let index = quiddity_core::Index::build(g).unwrap();
            g.create_entity("Alpha", &[]).unwrap();
            let zed = g.create_entity("Zed", &[]).unwrap();
            note(g, zed, "[[Alpha]]");
            for days in [3, 1, 2] {
                let date = today().checked_sub(days.days()).unwrap();
                let day = quiddity_core::ensure_journal(g, &index, date).unwrap();
                note(g, day, "[[Alpha]]");
            }
        });
        let refs = v.references(&id_of(&v, "Alpha")).unwrap();
        assert_eq!(
            titles(&refs),
            ["2026-10-25", "2026-10-24", "2026-10-23", "Zed"]
        );
    }

    #[test]
    fn unlinked_references_are_whole_word_prose_mentions() {
        let v = view_with(|g| {
            g.create_entity("Alpha", &[("alias", text("Omega"))])
                .unwrap();
            let beta = g.create_entity("Beta", &[]).unwrap();
            note(g, beta, "alpha is mentioned");
            note(g, beta, "Alphabet soup");
            note(g, beta, "[[Alpha]] is linked");
            note(g, beta, "[[Alpha Centauri]] is somewhere else");
            note(g, beta, "alias: ((alpha)) is a reference");
            let gamma = g
                .create_entity("Gamma", &[("type", text("alpha"))])
                .unwrap();
            note(g, gamma, "ALPHA, again!");
            let delta = g.create_entity("Delta", &[]).unwrap();
            note(g, delta, "omega here, via an alias");
            g.create_entity("Alpha beta", &[]).unwrap();
        });
        let refs = v.unlinked_references(&id_of(&v, "Alpha")).unwrap();
        assert_eq!(titles(&refs), ["Beta", "Delta", "Gamma"]);
        let text = |r: &Reference| texts(&r.blocks).join("|");
        assert_eq!(text(&refs[0]), "alpha is mentioned");
        assert_eq!(text(&refs[1]), "omega here, via an alias");
        assert_eq!(text(&refs[2]), "ALPHA, again!", "not the type:: property");
    }

    #[test]
    fn aliases_written_as_links_count_too() {
        let omega = Value::Link(Link {
            name: "Omega".into(),
            qualifiers: Vec::new(),
        });
        let v = view_with(|g| {
            g.create_entity("Alpha", &[("alias", Some(omega))]).unwrap();
            let beta = g.create_entity("Beta", &[]).unwrap();
            note(g, beta, "omega, by its alias");
        });
        let refs = v.unlinked_references(&id_of(&v, "Alpha")).unwrap();
        assert_eq!(titles(&refs), ["Beta"]);
    }

    #[test]
    fn a_block_that_links_is_not_also_an_unlinked_mention() {
        let v = view_with(|g| {
            g.create_entity("Alpha", &[]).unwrap();
            let beta = g.create_entity("Beta", &[]).unwrap();
            note(g, beta, "[[Alpha]], and alpha again in prose");
        });
        let alpha = id_of(&v, "Alpha");
        assert_eq!(titles(&v.references(&alpha).unwrap()), ["Beta"]);
        assert!(v.unlinked_references(&alpha).unwrap().is_empty());
    }

    #[test]
    fn journals_have_no_unlinked_references() {
        let v = view_with(|_| {});
        let today = v.ensure_journal(today()).unwrap();
        assert!(v.unlinked_references(&today.id).unwrap().is_empty());
    }

    #[test]
    fn the_index_follows_edits_and_imports() {
        let v = view_with(|g| {
            let a = g.create_entity("A", &[]).unwrap();
            note(g, a, "no links yet");
            g.create_entity("B", &[]).unwrap();
        });
        let b = id_of(&v, "B");
        assert!(v.references(&b).unwrap().is_empty());
        let block = v.entity(&id_of(&v, "A")).unwrap().blocks[0].id.clone();
        v.splice_text(&block, 0, 2, "see [[B]] --").unwrap();
        assert_eq!(titles(&v.references(&b).unwrap()), ["A"]);

        // A change made behind the view's back is noticed once it says so.
        let other = Graph::new();
        other.import(&v.graph().snapshot().unwrap()).unwrap();
        let c = other.create_block(None, None, "C").unwrap();
        v.graph().import(&other.snapshot().unwrap()).unwrap();
        v.changed();
        assert!(v.entities().unwrap().iter().any(|e| e.id == c.to_string()));
    }
}

mod edits {
    use super::*;

    #[test]
    fn typing_splices_in_utf16_units() {
        let (v, page, ids) = page(&["a😀b"]);
        v.splice_text(&ids[0], 3, 1, "B").unwrap();
        v.splice_text(&ids[0], 0, 0, ">").unwrap();
        assert_eq!(top_level(&v, &page), [">a😀B"]);
        assert!(v.splice_text(&ids[0], 9, 0, "x").is_err());
        assert!(v.splice_text("junk", 0, 0, "x").is_err());
    }

    #[test]
    fn a_blocks_text_is_read_as_it_is_now() {
        let (v, _page, ids) = page(&["one"]);
        assert_eq!(v.block_text(&ids[0]).unwrap(), "one");
        v.splice_text(&ids[0], 3, 0, " two").unwrap();
        assert_eq!(v.block_text(&ids[0]).unwrap(), "one two");
        assert!(matches!(v.block_text("junk"), Err(Error::NotABlock(_))));
    }

    #[test]
    fn splitting_moves_the_tail_to_a_new_next_sibling() {
        let (v, page, ids) = page(&["hello world", "next"]);
        let new = v.split(&ids[0], 5).unwrap();
        assert_eq!(top_level(&v, &page), ["hello", " world", "next"]);
        assert_eq!(v.entity(&page).unwrap().blocks[1].id, new);
        let end = v.split(&ids[0], 5).unwrap();
        assert_eq!(top_level(&v, &page), ["hello", "", " world", "next"]);
        assert_ne!(end, new);
    }

    #[test]
    fn splitting_a_parent_makes_the_tail_its_first_child() {
        let (v, page, ids) = page(&["parent"]);
        let child = v.create_block(&ids[0], None, "child").unwrap();
        let new = v.split(&ids[0], 3).unwrap();
        let e = v.entity(&page).unwrap();
        assert_eq!(e.blocks.len(), 1);
        assert_eq!(e.blocks[0].text, "par");
        assert_eq!(texts(&e.blocks[0].children), ["ent", "child"]);
        assert_eq!(e.blocks[0].children[0].id, new);
        assert_eq!(e.blocks[0].children[1].id, child);
    }

    #[test]
    fn splitting_must_land_between_characters() {
        let (v, page, ids) = page(&["a😀b"]);
        assert!(matches!(v.split(&ids[0], 2), Err(Error::BadOffset(2))));
        assert!(matches!(v.split(&ids[0], 9), Err(Error::BadOffset(9))));
        assert_eq!(top_level(&v, &page), ["a😀b"], "nothing changed");
        v.split(&ids[0], 3).unwrap();
        assert_eq!(top_level(&v, &page), ["a😀", "b"]);
    }

    #[test]
    fn indenting_nests_under_the_sibling_before() {
        let (v, page, ids) = page(&["one", "two", "three"]);
        assert!(!v.indent(&ids[0]).unwrap(), "nothing before the first");
        assert!(v.indent(&ids[1]).unwrap());
        assert!(
            v.indent(&ids[2]).unwrap(),
            "under the one before, which is now one"
        );
        let e = v.entity(&page).unwrap();
        assert_eq!(texts(&e.blocks), ["one"]);
        assert_eq!(texts(&e.blocks[0].children), ["two", "three"]);
    }

    #[test]
    fn outdenting_follows_the_parent() {
        let (v, page, ids) = page(&["one", "two"]);
        let kid = v.create_block(&ids[0], None, "kid").unwrap();
        assert!(!v.outdent(&ids[0]).unwrap(), "already at the top");
        assert!(v.outdent(&kid).unwrap());
        assert_eq!(top_level(&v, &page), ["one", "kid", "two"]);
    }

    #[test]
    fn blocks_swap_with_their_neighbours() {
        let (v, page, ids) = page(&["a", "b", "c"]);
        assert!(!v.move_sibling(&ids[0], -1).unwrap());
        assert!(!v.move_sibling(&ids[2], 1).unwrap());
        assert!(v.move_sibling(&ids[0], 1).unwrap());
        assert_eq!(top_level(&v, &page), ["b", "a", "c"]);
        assert!(v.move_sibling(&ids[2], -1).unwrap());
        assert_eq!(top_level(&v, &page), ["b", "c", "a"]);
    }

    #[test]
    fn merging_joins_a_block_to_the_one_before_in_reading_order() {
        let (v, page, ids) = page(&["first", "second"]);
        let kid = v.create_block(&ids[0], None, "kid").unwrap();
        // "second" follows "kid" in reading order, not "first".
        let merged = v.merge_into_previous(&ids[1]).unwrap().unwrap();
        assert_eq!(merged, Merged { into: kid, at: 3 });
        let e = v.entity(&page).unwrap();
        assert_eq!(texts(&e.blocks), ["first"]);
        assert_eq!(texts(&e.blocks[0].children), ["kidsecond"]);
    }

    #[test]
    fn merging_leaves_parents_and_first_blocks_alone() {
        let (v, page, ids) = page(&["first", "second"]);
        v.create_block(&ids[1], None, "kid").unwrap();
        assert_eq!(
            v.merge_into_previous(&ids[0]).unwrap(),
            None,
            "nothing before it"
        );
        assert_eq!(
            v.merge_into_previous(&ids[1]).unwrap(),
            None,
            "it has children"
        );
        assert_eq!(top_level(&v, &page), ["first", "second"]);
    }

    #[test]
    fn an_empty_block_merges_away_leaving_the_caret_at_the_end_before() {
        let (v, page, ids) = page(&["keep", ""]);
        let merged = v.merge_into_previous(&ids[1]).unwrap().unwrap();
        assert_eq!(
            merged,
            Merged {
                into: ids[0].clone(),
                at: 4
            }
        );
        assert_eq!(top_level(&v, &page), ["keep"]);
    }

    #[test]
    fn new_entities_start_with_an_empty_block() {
        let v = view_with(|_| {});
        let id = v.create_entity("Fresh").unwrap();
        let e = v.entity(&id).unwrap();
        assert_eq!(e.title, "Fresh");
        assert_eq!(texts(&e.blocks), [""]);
        assert_eq!(v.entities().unwrap().len(), 1, "and the index saw it");
    }

    #[test]
    fn edits_travel_as_updates() {
        let (v, page, ids) = page(&["x"]);
        let replica = Graph::blank();
        replica.import(&v.graph().snapshot().unwrap()).unwrap();
        let before = v.graph().version();
        v.splice_text(&ids[0], 1, 0, "y").unwrap();
        v.split(&ids[0], 1).unwrap();
        replica
            .import(&v.graph().updates_since(&before).unwrap())
            .unwrap();
        assert_eq!(top_level(&View::new(replica).unwrap(), &page), ["x", "y"]);
    }
}

mod text {
    use super::super::query::{contains_word, without_links};

    #[test]
    fn links_and_block_refs_are_blanked_where_they_are() {
        assert_eq!(
            without_links("see [[Not alpha]] ok ((id)) end"),
            "see   ok   end"
        );
        assert_eq!(without_links("[[a]][[b]]tail"), "  tail");
        assert_eq!(without_links("an [[unclosed link"), "an [[unclosed link");
        assert_eq!(
            without_links("a ((open and [[x]] done"),
            "a ((open and   done"
        );
        assert_eq!(without_links("é[[é]]é"), "é é");
    }

    #[test]
    fn words_are_whole() {
        assert!(contains_word("an alpha here", "alpha"));
        assert!(contains_word("alpha", "alpha"));
        assert!(contains_word("(alpha)", "alpha"));
        assert!(!contains_word("alphabet", "alpha"));
        assert!(!contains_word("an_alpha", "alpha"));
        assert!(!contains_word("alphas", "alpha"));
        assert!(
            contains_word("alphabet alpha", "alpha"),
            "a later whole word counts"
        );
        assert!(contains_word("éalpha alpha", "alpha"));
    }
}
