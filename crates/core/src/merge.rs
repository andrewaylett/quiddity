use std::collections::BTreeSet;

use crate::{BlockId, Graph, Index, Result, canonical_value};

/// Folds duplicate entities (those sharing an [`crate::Identity`]) into the
/// one with the lowest ID, by moving their content across. Returns the number
/// of blocks moved.
///
/// Duplicates are never deleted. A device that hasn't yet synced may still
/// be adding to one, and anything it adds under a deleted block would be
/// lost. Instead the index hides them, and running this again (after each
/// sync) moves anything new across. Every device makes the same moves, so
/// concurrent merges converge.
pub fn merge_duplicates(graph: &Graph, index: &Index) -> Result<usize> {
    let mut moved = 0;
    for group in index.duplicates() {
        let (&keep, dups) = group
            .split_first()
            .expect("duplicate groups have two or more members");
        for &dup in dups {
            let props = graph.properties_block(dup)?;
            for child in graph.children(dup) {
                if Some(child) != props {
                    graph.move_block(child, Some(keep), None)?;
                    moved += 1;
                }
            }
        }
    }
    graph.commit();
    Ok(moved)
}

/// Whether `id` is a duplicate with nothing left that its canonical entity
/// doesn't have, so it can be left out of exports.
pub(crate) fn is_spent_duplicate(graph: &Graph, index: &Index, id: BlockId) -> Result<bool> {
    let keep = index.canonical(id);
    if keep == id || graph.children(id).len() > usize::from(graph.properties_block(id)?.is_some()) {
        return Ok(false);
    }
    let props = |id| -> Result<BTreeSet<_>> {
        Ok(graph
            .entity_properties(id)?
            .into_iter()
            .map(|p| {
                (
                    quiddity_syntax::normalise_name(&p.key),
                    p.value.as_ref().map(canonical_value),
                )
            })
            .collect())
    };
    Ok(props(id)?.is_subset(&props(keep)?))
}

// Merging is property-tested across replicas in tests/merge.rs.
