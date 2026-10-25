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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ensure_journal;
    use jiff::civil::Date;

    fn sync(a: &Graph, b: &Graph) -> Result<()> {
        b.import(&a.updates_since(&b.version())?)?;
        a.import(&b.updates_since(&a.version())?)?;
        Ok(())
    }

    #[test]
    fn concurrent_journals_converge() -> Result<()> {
        let date: Date = "2026-10-25".parse().unwrap();
        let laptop = Graph::new();
        let phone = Graph::blank();
        sync(&laptop, &phone)?;

        let on_laptop = ensure_journal(&laptop, &Index::build(&laptop)?, date)?;
        laptop.create_block(Some(on_laptop), None, "written on the laptop")?;
        let on_phone = ensure_journal(&phone, &Index::build(&phone)?, date)?;
        phone.create_block(Some(on_phone), None, "written on the phone")?;
        laptop.commit();
        phone.commit();
        sync(&laptop, &phone)?;

        // Both devices merge concurrently, and the phone keeps writing to its
        // own copy before it hears about the merge.
        assert_eq!(merge_duplicates(&laptop, &Index::build(&laptop)?)?, 1);
        assert_eq!(merge_duplicates(&phone, &Index::build(&phone)?)?, 1);
        let keep = Index::build(&laptop)?.journal(date).unwrap();
        let dup = if keep == on_laptop {
            on_phone
        } else {
            on_laptop
        };
        phone.create_block(Some(dup), None, "late")?;
        phone.commit();
        sync(&laptop, &phone)?;
        merge_duplicates(&laptop, &Index::build(&laptop)?)?;
        sync(&laptop, &phone)?;

        for graph in [&laptop, &phone] {
            let index = Index::build(graph)?;
            assert_eq!(index.journal(date), Some(keep));
            let mut texts: Vec<_> = graph.children(keep)[1..]
                .iter()
                .map(|&b| graph.text(b))
                .collect::<Result<_>>()?;
            texts.sort();
            assert_eq!(
                texts,
                ["late", "written on the laptop", "written on the phone"]
            );
            assert!(is_spent_duplicate(graph, &index, dup)?);
            assert!(!is_spent_duplicate(graph, &index, keep)?);
        }
        Ok(())
    }
}
