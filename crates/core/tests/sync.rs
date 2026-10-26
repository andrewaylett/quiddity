//! Replicas editing concurrently and syncing in arbitrary orders.

mod common;

use common::{Op, apply, layout, op, sync, sync_all};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::Index;
use quiddity_core::{Graph, SCHEMA_VERSION};

const REPLICAS: usize = 3;

#[derive(Debug, Clone)]
enum Step {
    Edit(usize, Op),
    Sync(usize, usize),
    /// The device restarts, reloading from its own snapshot.
    Restart(usize),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        6 => (0..REPLICAS, op()).prop_map(|(r, op)| Step::Edit(r, op)),
        2 => (0..REPLICAS, 0..REPLICAS).prop_map(|(a, b)| Step::Sync(a, b)),
        1 => (0..REPLICAS).prop_map(Step::Restart),
    ]
}

/// One replica that made the graph, one that joined from its snapshot, and
/// one that started blank and only ever receives updates.
fn replicas() -> Vec<Graph> {
    let origin = Graph::new();
    let joined = Graph::from_snapshot(&origin.snapshot().unwrap()).unwrap();
    vec![origin, joined, Graph::blank()]
}

/// Edits that only add: no deletions and no rewrites of text, so nothing
/// any replica does should be lost.
#[derive(Debug, Clone)]
enum Addition {
    Create {
        replica: usize,
        parent: Option<Index>,
    },
    Append {
        replica: usize,
        block: Index,
    },
    Move {
        replica: usize,
        block: Index,
        parent: Option<Index>,
    },
    Sync(usize, usize),
}

fn addition() -> impl Strategy<Value = Addition> {
    let parent = proptest::option::weighted(0.8, any::<Index>());
    prop_oneof![
        3 => (0..REPLICAS, parent.clone()).prop_map(|(replica, parent)| Addition::Create { replica, parent }),
        3 => (0..REPLICAS, any::<Index>()).prop_map(|(replica, block)| Addition::Append { replica, block }),
        2 => (0..REPLICAS, any::<Index>(), parent).prop_map(|(replica, block, parent)| Addition::Move { replica, block, parent }),
        2 => (0..REPLICAS, 0..REPLICAS).prop_map(|(a, b)| Addition::Sync(a, b)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// However edits and syncs interleave, once everyone has synced all
    /// replicas hold the same graph, and a snapshot reproduces it.
    #[test]
    fn replicas_converge(steps in vec(step(), 1..60)) {
        let mut replicas = replicas();
        for step in &steps {
            match *step {
                Step::Edit(r, ref op) => apply(&replicas[r], op),
                Step::Sync(a, b) => sync(&replicas[a], &replicas[b]).unwrap(),
                Step::Restart(r) => {
                    replicas[r] = Graph::from_snapshot(&replicas[r].snapshot().unwrap()).unwrap();
                }
            }
        }
        sync_all(&replicas).unwrap();

        let expected = layout(&replicas[0]);
        for replica in &replicas {
            prop_assert_eq!(replica.schema_version(), Some(SCHEMA_VERSION));
            prop_assert_eq!(&layout(replica), &expected);
            let reloaded = Graph::from_snapshot(&replica.snapshot().unwrap()).unwrap();
            prop_assert_eq!(&layout(&reloaded), &expected);
        }
    }

    /// Without deletions or rewrites, every block any replica created and
    /// every piece of text any replica appended survives, wherever
    /// concurrent moves put it.
    #[test]
    fn concurrent_additions_survive(steps in vec(addition(), 1..60)) {
        let replicas = replicas();
        let mut created = 0;
        let mut appended: Vec<(quiddity_core::BlockId, String)> = Vec::new();
        for step in &steps {
            let pick = |graph: &Graph, i: &Index| {
                let blocks = graph.blocks();
                (!blocks.is_empty()).then(|| blocks[i.index(blocks.len())])
            };
            match step {
                Addition::Create { replica, parent } => {
                    let graph = &replicas[*replica];
                    let parent = parent.as_ref().and_then(|p| pick(graph, p));
                    graph.create_block(parent, None, &format!("<{created}>")).unwrap();
                    created += 1;
                }
                Addition::Append { replica, block } => {
                    let graph = &replicas[*replica];
                    let Some(block) = pick(graph, block) else { continue };
                    let token = format!("+{};", appended.len());
                    graph.set_text(block, &format!("{}{token}", graph.text(block).unwrap())).unwrap();
                    appended.push((block, token));
                }
                Addition::Move { replica, block, parent } => {
                    let graph = &replicas[*replica];
                    let (Some(block), parent) = (pick(graph, block), parent.as_ref().and_then(|p| pick(graph, p))) else { continue };
                    // A move under its own descendant is refused locally.
                    let _ = graph.move_block(block, parent, None);
                }
                Addition::Sync(a, b) => sync(&replicas[*a], &replicas[*b]).unwrap(),
            }
            replicas.iter().for_each(Graph::commit);
        }
        sync_all(&replicas).unwrap();

        // Each block's text is its creation token followed by everything
        // appended to it, in some order, each exactly once.
        let expected: Vec<(String, Vec<&str>)> = replicas[0]
            .blocks()
            .into_iter()
            .map(|b| {
                let mine = appended.iter().filter(|(on, _)| *on == b).map(|(_, t)| t.as_str()).collect();
                (replicas[0].text(b).unwrap(), mine)
            })
            .collect();
        prop_assert_eq!(expected.len(), created, "every created block survives");
        for (text, tokens) in &expected {
            let origin = &text[..=text.find('>').unwrap()];
            let mut rest = text[origin.len()..].to_owned();
            for token in tokens {
                prop_assert_eq!(rest.matches(token).count(), 1, "{:?} in {:?}", token, text);
                rest = rest.replacen(token, "", 1);
            }
            prop_assert_eq!(rest, "", "{:?} has unexpected text", text);
        }
        for replica in &replicas[1..] {
            prop_assert_eq!(layout(replica), layout(&replicas[0]));
        }
    }
}
