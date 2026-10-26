//! The open graph on disk.
//!
//! The UI holds its own replica and answers its own queries (see
//! `quiddity-wasm`). The shell's part is to hand over the graph at startup,
//! apply the updates the UI's edits make, and keep the result safe. It knows
//! nothing about Tauri, so it can be tested on its own; `lib.rs` exposes it as
//! commands.

use std::path::Path;

use quiddity_core::Graph;
use quiddity_node::store::Store;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Store(#[from] quiddity_node::StoreError),
    #[error("{0}")]
    Core(#[from] quiddity_core::Error),
    #[error("expected the updates as raw bytes")]
    NotBytes,
}

impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug)]
pub struct Session {
    store: Store,
    graph: Graph,
}

impl Session {
    /// Opens the graph in `dir`, creating an empty one if there is none.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let store = Store::new(dir);
        let graph = match store.load() {
            Ok(graph) => graph,
            Err(quiddity_node::StoreError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                store.init(false)?
            }
            Err(e) => return Err(e.into()),
        };
        Ok(Self { store, graph })
    }

    /// The whole graph, for the UI to start its replica from.
    pub fn snapshot(&self) -> Result<Vec<u8>, Error> {
        Ok(self.graph.snapshot()?)
    }

    /// Applies updates from the UI, and saves. Updates that arrive out of
    /// order are held until what they depend on does.
    pub fn apply(&self, updates: &[u8]) -> Result<(), Error> {
        self.graph.import(updates)?;
        self.store.save(&self.graph)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_graph_when_there_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph");
        let session = Session::open(&path).unwrap();
        assert!(path.join("graph.loro").exists());
        Graph::from_snapshot(&session.snapshot().unwrap()).unwrap();
    }

    /// The UI's side of it: start from the snapshot, edit, send the updates.
    #[test]
    fn the_uis_edits_are_applied_and_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let session = Session::open(dir.path()).unwrap();
        let ui = Graph::from_snapshot(&session.snapshot().unwrap()).unwrap();
        let before = ui.version();
        let page = ui.create_entity("Typed in the UI", &[]).unwrap();
        ui.commit();
        session.apply(&ui.updates_since(&before).unwrap()).unwrap();

        let reopened = Session::open(dir.path()).unwrap();
        let graph = Graph::from_snapshot(&reopened.snapshot().unwrap()).unwrap();
        assert_eq!(graph.entity_name(page).unwrap(), "Typed in the UI");
    }

    #[test]
    fn nonsense_is_refused_and_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let session = Session::open(dir.path()).unwrap();
        let before = session.snapshot().unwrap();
        assert!(session.apply(b"not an update").is_err());
        assert_eq!(session.snapshot().unwrap(), before);
    }
}
