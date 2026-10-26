//! The open graph and the read-only queries the UI makes of it.
//!
//! This knows nothing about Tauri, so it can be tested on its own; `lib.rs`
//! exposes it as commands. The types here are the wire format and mirror
//! `apps/ui/lib/model.ts`.

use std::collections::BTreeMap;
use std::path::Path;

use jiff::Zoned;
use quiddity_core::schema::{Typed, coerce};
use quiddity_core::syntax::{format_link, parse_block};
use quiddity_core::{BlockId, Graph, Index, Resolution, ensure_journal};
use quiddity_node::store::Store;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Store(#[from] quiddity_node::StoreError),
    #[error("{0}")]
    Core(#[from] quiddity_core::Error),
    #[error("{0:?} is not an entity")]
    NotAnEntity(String),
}

impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EntityRef {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub id: String,
    pub text: String,
    pub children: Vec<Block>,
}

/// Where a `[[link]]` goes.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LinkTarget {
    Entity { id: String },
    Ambiguous { ids: Vec<String> },
    Missing,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub title: String,
    pub blocks: Vec<Block>,
    /// Every link in the entity's blocks, keyed by the text between the
    /// brackets as the syntax crate writes it. Until the UI parses text
    /// itself (the syntax crate compiled to WASM), links written some other
    /// way are not found here and render as unresolved.
    pub links: BTreeMap<String, LinkTarget>,
}

#[derive(Debug)]
pub struct Session {
    store: Store,
    graph: Graph,
    index: Index,
}

impl Session {
    /// Opens the graph in `dir`, creating an empty one if there is none.
    pub fn open(dir: &Path) -> Result<Self> {
        let store = Store::new(dir);
        let graph = match store.load() {
            Ok(graph) => graph,
            Err(quiddity_node::StoreError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                store.init(false)?
            }
            Err(e) => return Err(e.into()),
        };
        let index = Index::build(&graph)?;
        Ok(Self {
            store,
            graph,
            index,
        })
    }

    fn id(&self, id: &str) -> Result<BlockId> {
        let bad = || Error::NotAnEntity(id.to_owned());
        let id: BlockId = id.parse().map_err(|_| bad())?;
        if !self.graph.contains(id) || self.graph.parent(id).is_some() {
            return Err(bad());
        }
        Ok(self.index.canonical(id))
    }

    fn entity_ref(&self, id: BlockId) -> Result<EntityRef> {
        Ok(EntityRef {
            id: id.to_string(),
            title: self.graph.entity_name(id)?,
        })
    }

    /// Entities that are not merged into another, by name.
    fn live_entities(&self) -> Result<Vec<EntityRef>> {
        let mut refs = self
            .index
            .entities()
            .filter(|e| self.index.canonical(e.id) == e.id)
            .map(|e| self.entity_ref(e.id))
            .collect::<Result<Vec<_>>>()?;
        refs.sort_by_cached_key(|e| (e.title.to_lowercase(), e.id.clone()));
        Ok(refs)
    }

    pub fn entities(&self) -> Result<Vec<EntityRef>> {
        self.live_entities()
    }

    /// Entities whose `favourite` property is true.
    pub fn favourites(&self) -> Result<Vec<EntityRef>> {
        let schema = self.index.schema("favourite");
        let mut favourites = Vec::new();
        for entity in self.index.entities() {
            let favourite = entity
                .values("favourite")
                .any(|v| matches!(coerce(v, &schema), Ok(Typed::Boolean(true))));
            if favourite && self.index.canonical(entity.id) == entity.id {
                favourites.push(self.entity_ref(entity.id)?);
            }
        }
        favourites.sort_by_cached_key(|e| (e.title.to_lowercase(), e.id.clone()));
        Ok(favourites)
    }

    /// Today's journal page, created (and saved) if there isn't one.
    pub fn today(&mut self) -> Result<EntityRef> {
        let date = Zoned::now().date();
        let id = match self.index.journal(date) {
            Some(id) => id,
            None => {
                let id = ensure_journal(&self.graph, &self.index, date)?;
                self.store.save(&self.graph)?;
                self.index = Index::build(&self.graph)?;
                id
            }
        };
        self.entity_ref(self.index.canonical(id))
    }

    pub fn entity(&self, id: &str) -> Result<Entity> {
        let id = self.id(id)?;
        let mut links = BTreeMap::new();
        let blocks = self
            .graph
            .children(id)
            .into_iter()
            .map(|child| self.block(child, &mut links))
            .collect::<Result<_>>()?;
        Ok(Entity {
            id: id.to_string(),
            title: self.graph.entity_name(id)?,
            blocks,
            links,
        })
    }

    fn block(&self, id: BlockId, links: &mut BTreeMap<String, LinkTarget>) -> Result<Block> {
        let text = self.graph.text(id)?;
        for link in parse_block(&text).links {
            let written = format_link(&link);
            let key = written
                .strip_prefix("[[")
                .and_then(|s| s.strip_suffix("]]"))
                .unwrap_or(&written)
                .to_owned();
            let target = match self.index.resolve(&link) {
                Resolution::Unique(id) => LinkTarget::Entity {
                    id: self.index.canonical(id).to_string(),
                },
                Resolution::Ambiguous(ids) => LinkTarget::Ambiguous {
                    ids: ids.iter().map(ToString::to_string).collect(),
                },
                Resolution::Missing => LinkTarget::Missing,
            };
            links.insert(key, target);
        }
        let children = self
            .graph
            .children(id)
            .into_iter()
            .map(|child| self.block(child, links))
            .collect::<Result<_>>()?;
        Ok(Block {
            id: id.to_string(),
            text,
            children,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiddity_core::syntax::Value;

    fn text(s: &str) -> Option<Value> {
        Some(Value::Text(s.to_owned()))
    }

    /// A graph on disk with a few entities, reopened as a session.
    fn session() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let graph = store.init(false).unwrap();
        graph.create_entity("Zebra", &[]).unwrap();
        graph
            .create_entity("apple", &[("favourite", text("true"))])
            .unwrap();
        graph
            .create_entity("Mango", &[("favourite", text("no"))])
            .unwrap();
        let quiddity = graph
            .create_entity("Quiddity", &[("favourite", text("Yes"))])
            .unwrap();
        let note = graph
            .create_block(Some(quiddity), None, "Built on [[apple]], [[Nope]]")
            .unwrap();
        graph.create_block(Some(note), None, "nested").unwrap();
        graph.commit();
        store.save(&graph).unwrap();
        let session = Session::open(dir.path()).unwrap();
        (dir, session)
    }

    #[test]
    fn creates_a_graph_when_there_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let graph = dir.path().join("graph");
        assert!(
            Session::open(&graph)
                .unwrap()
                .entities()
                .unwrap()
                .is_empty()
        );
        assert!(graph.join("graph.loro").exists());
    }

    #[test]
    fn lists_entities_by_name_ignoring_case() {
        let (_dir, s) = session();
        let names: Vec<_> = s.entities().unwrap().into_iter().map(|e| e.title).collect();
        assert_eq!(names, ["apple", "Mango", "Quiddity", "Zebra"]);
    }

    #[test]
    fn favourites_come_from_the_property() {
        let (_dir, s) = session();
        let names: Vec<_> = s
            .favourites()
            .unwrap()
            .into_iter()
            .map(|e| e.title)
            .collect();
        assert_eq!(names, ["apple", "Quiddity"]);
    }

    #[test]
    fn an_entity_has_nested_blocks_and_resolved_links() {
        let (_dir, s) = session();
        let id = s
            .entities()
            .unwrap()
            .into_iter()
            .find(|e| e.title == "Quiddity")
            .unwrap()
            .id;
        let e = s.entity(&id).unwrap();
        assert_eq!(e.title, "Quiddity");
        assert_eq!(e.blocks.len(), 2, "the properties block, then the note");
        assert_eq!(e.blocks[1].text, "Built on [[apple]], [[Nope]]");
        assert_eq!(e.blocks[1].children[0].text, "nested");
        let apple = s.entities().unwrap().remove(0).id;
        assert_eq!(e.links["apple"], LinkTarget::Entity { id: apple });
        assert_eq!(e.links["Nope"], LinkTarget::Missing);
    }

    #[test]
    fn only_entities_can_be_opened() {
        let (_dir, s) = session();
        let id = s.entities().unwrap().remove(0).id;
        let block = s.entity(&id).unwrap().blocks[0].id.clone();
        assert!(s.entity(&block).is_err(), "a block is not an entity");
        assert!(s.entity("nonsense").is_err());
    }

    #[test]
    fn today_is_created_once_and_saved() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::open(dir.path()).unwrap();
        let first = s.today().unwrap();
        assert_eq!(s.today().unwrap(), first);
        let reopened = Session::open(dir.path()).unwrap();
        assert_eq!(reopened.entities().unwrap(), [first]);
    }
}
