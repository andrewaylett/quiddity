//! The whole pipeline: a graph built by random edits, exported to a
//! directory, imported into a fresh graph, and compared tree by tree.
//!
//! The comparison walks both graphs through the graph API alone, not the
//! outline code the export uses, so a mistake shared by export and import
//! can't cancel itself out.

mod common;

use common::{apply, forest, op};
use proptest::collection::vec;
use proptest::prelude::*;
use quiddity_core::Graph;
use quiddity_core::files::{export_dir, import_dir};

proptest! {
    #![proptest_config(common::config(64))]

    #[test]
    fn export_then_import_preserves_the_graph(ops in vec(op(), 1..40)) {
        let graph = Graph::new();
        for op in &ops {
            apply(&graph, op);
        }

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("export");
        let files = export_dir(&graph, &out).unwrap();
        prop_assert_eq!(files, graph.roots().len());

        let copy = Graph::new();
        import_dir(&copy, &out).unwrap();
        // Files are read in name order, so entities may come back in a
        // different order; everything within an entity must be identical.
        let mut before = forest(&graph);
        let mut after = forest(&copy);
        before.sort();
        after.sort();
        prop_assert_eq!(after, before);
    }
}
