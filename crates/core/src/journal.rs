use jiff::civil::Date;
use quiddity_syntax::Value;

use crate::{BlockId, Graph, Index, Result};

/// Returns the journal page for `date`, creating it if the index has none.
///
/// Two devices doing this offline will each create a page; once they sync,
/// [`crate::merge_duplicates`] folds them together.
pub fn ensure_journal(graph: &Graph, index: &Index, date: Date) -> Result<BlockId> {
    if let Some(id) = index.journal(date) {
        return Ok(id);
    }
    let date = date.to_string();
    graph.create_entity(
        &date,
        &[
            ("type", Some(Value::Text("journal".into()))),
            ("date", Some(Value::Text(date.clone()))),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_once() -> Result<()> {
        let graph = Graph::new();
        let date: Date = "2026-10-25".parse().unwrap();
        let day = ensure_journal(&graph, &Index::build(&graph)?, date)?;
        assert_eq!(graph.entity_name(day)?, "2026-10-25");
        assert_eq!(
            graph.text(graph.children(day)[0])?,
            "type:: journal\ndate:: 2026-10-25"
        );
        assert_eq!(ensure_journal(&graph, &Index::build(&graph)?, date)?, day);
        Ok(())
    }
}
