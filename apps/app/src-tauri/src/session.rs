//! The open graph and the read-only queries the UI makes of it.
//!
//! This knows nothing about Tauri, so it can be tested on its own; `lib.rs`
//! exposes it as commands. The types here are the wire format and mirror
//! `apps/ui/lib/model.ts`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use jiff::Zoned;
use quiddity_core::schema::{Typed, coerce};
use quiddity_core::syntax::{Value, format_link, is_properties_block, parse_block};
use quiddity_core::{BlockId, Graph, Identity, Index, Resolution, ensure_journal};
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

/// The blocks of one entity that mention another.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub source: EntityRef,
    /// Each with its children, for context.
    pub blocks: Vec<Block>,
    /// As [`Entity::links`], for the links in these blocks.
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

    /// Journal pages from today back, newest first. Today's is created (and
    /// saved) if there isn't one.
    pub fn journals(&mut self) -> Result<Vec<EntityRef>> {
        let today = Zoned::now().date();
        if self.index.journal(today).is_none() {
            ensure_journal(&self.graph, &self.index, today)?;
            self.store.save(&self.graph)?;
            self.index = Index::build(&self.graph)?;
        }
        let mut days: Vec<_> = self
            .index
            .entities()
            .filter(|e| self.index.canonical(e.id) == e.id)
            .filter_map(|e| match e.identity() {
                Some(Identity::Journal(date)) if date <= today => Some((date, e.id)),
                _ => None,
            })
            .collect();
        days.sort_by(|a, b| b.cmp(a));
        days.into_iter()
            .map(|(_, id)| self.entity_ref(id))
            .collect()
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

    /// Blocks that link to the entity, by the entity they are in.
    pub fn references(&self, id: &str) -> Result<Vec<Reference>> {
        let target = self.id(id)?;
        let sources = self
            .index
            .backlinks(target)
            .iter()
            .map(|b| b.source)
            .collect();
        self.grouped(target, sources)
    }

    /// Blocks that mention the entity's name or an alias in plain text,
    /// without linking to it.
    pub fn unlinked_references(&self, id: &str) -> Result<Vec<Reference>> {
        let target = self.id(id)?;
        let Some(info) = self.index.entity(target) else {
            return Ok(Vec::new());
        };
        if matches!(info.identity(), Some(Identity::Journal(_))) {
            return Ok(Vec::new());
        }
        let names: Vec<String> = std::iter::once(info.name.clone())
            .chain(
                info.values("alias")
                    .flat_map(|v| match v {
                        Value::List(items) => items.iter().collect(),
                        v => vec![v],
                    })
                    .filter_map(|v| match v {
                        Value::Text(s) => Some(s.clone()),
                        Value::Link(link) => Some(link.name.clone()),
                        _ => None,
                    }),
            )
            .map(|n| n.trim().to_lowercase())
            .filter(|n| !n.is_empty())
            .collect();
        let linked: BTreeSet<_> = self
            .index
            .backlinks(target)
            .iter()
            .map(|b| b.source)
            .collect();
        let mut found = BTreeSet::new();
        for block in self.graph.blocks() {
            // Entity names aren't prose, and property values are structured.
            if self.graph.parent(block).is_none() || linked.contains(&block) {
                continue;
            }
            let text = self.graph.text(block)?;
            if is_properties_block(&text) {
                continue;
            }
            let prose = without_links(&text).to_lowercase();
            if names.iter().any(|n| contains_word(&prose, n)) {
                found.insert(block);
            }
        }
        self.grouped(target, found)
    }

    /// Groups blocks by entity, leaving out the target's own and any block
    /// already shown inside another.
    fn grouped(&self, target: BlockId, blocks: BTreeSet<BlockId>) -> Result<Vec<Reference>> {
        let mut by_entity: BTreeMap<BlockId, Vec<BlockId>> = BTreeMap::new();
        for &block in &blocks {
            let entity = self.index.canonical(self.graph.entity_of(block));
            let nested = std::iter::successors(self.graph.parent(block), |&p| self.graph.parent(p))
                .any(|ancestor| blocks.contains(&ancestor));
            if entity != target && block != self.graph.entity_of(block) && !nested {
                by_entity.entry(entity).or_default().push(block);
            }
        }
        let mut groups = Vec::new();
        for (entity, blocks) in by_entity {
            let mut links = BTreeMap::new();
            let blocks = blocks
                .into_iter()
                .map(|b| self.block(b, &mut links))
                .collect::<Result<_>>()?;
            let date = match self.index.entity(entity).and_then(|e| e.identity()) {
                Some(Identity::Journal(date)) => Some(date),
                _ => None,
            };
            let source = self.entity_ref(entity)?;
            groups.push((
                date,
                Reference {
                    source,
                    blocks,
                    links,
                },
            ));
        }
        // Recent days first, then everything else by name.
        groups.sort_by_cached_key(|(date, g)| {
            (
                date.is_none(),
                std::cmp::Reverse(*date),
                g.source.title.to_lowercase(),
                g.source.id.clone(),
            )
        });
        Ok(groups.into_iter().map(|(_, g)| g).collect())
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

/// `text` with `[[links]]` and `((block refs))` blanked out, so a name inside
/// one doesn't count as a plain mention.
fn without_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let skipped = ["[[", "(("].iter().find_map(|open| {
            let close = if *open == "[[" { "]]" } else { "))" };
            let inner = rest.strip_prefix(open)?;
            inner.find(close).map(|end| &inner[end + 2..])
        });
        match skipped {
            Some(after) => {
                out.push(' ');
                rest = after;
            }
            None => {
                let c = rest.chars().next().expect("rest is not empty");
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

/// Whether `name` occurs in `text` as whole words.
fn contains_word(text: &str, name: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(name).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + name.len()..].chars().next();
        !before.is_some_and(word) && !after.is_some_and(word)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiddity_core::syntax::Link;

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

    /// A session on a graph that `build` has filled.
    fn open_with(build: impl FnOnce(&Graph)) -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let graph = store.init(false).unwrap();
        build(&graph);
        graph.commit();
        store.save(&graph).unwrap();
        let session = Session::open(dir.path()).unwrap();
        (dir, session)
    }

    fn note(graph: &Graph, entity: BlockId, text: &str) -> BlockId {
        graph.create_block(Some(entity), None, text).unwrap()
    }

    fn id_of(s: &Session, title: &str) -> String {
        s.entities()
            .unwrap()
            .into_iter()
            .find(|e| e.title == title)
            .unwrap()
            .id
    }

    fn titles(refs: &[Reference]) -> Vec<&str> {
        refs.iter().map(|r| r.source.title.as_str()).collect()
    }

    #[test]
    fn journals_run_newest_first_from_today_and_are_saved() {
        use jiff::ToSpan;
        let today = Zoned::now().date();
        let day = |d: jiff::civil::Date| d.to_string();
        let (dir, mut s) = open_with(|g| {
            let index = Index::build(g).unwrap();
            for date in [
                today.checked_sub(5.days()).unwrap(),
                today.checked_sub(1.day()).unwrap(),
                today.checked_add(1.day()).unwrap(),
            ] {
                ensure_journal(g, &index, date).unwrap();
            }
            g.create_entity("Not a journal", &[]).unwrap();
        });
        let titles = |s: &mut Session| -> Vec<String> {
            s.journals().unwrap().into_iter().map(|e| e.title).collect()
        };
        let expected = [
            day(today),
            day(today.checked_sub(1.day()).unwrap()),
            day(today.checked_sub(5.days()).unwrap()),
        ];
        assert_eq!(titles(&mut s), expected, "today created, tomorrow left out");
        assert_eq!(titles(&mut s), expected, "and created only once");
        let mut reopened = Session::open(dir.path()).unwrap();
        assert_eq!(titles(&mut reopened), expected, "and saved");
    }

    #[test]
    fn references_are_grouped_by_entity_without_nesting_or_self_links() {
        let (_dir, s) = open_with(|g| {
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
        let refs = s.references(&id_of(&s, "Alpha")).unwrap();
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
    fn unlinked_references_are_whole_word_prose_mentions() {
        let (_dir, s) = open_with(|g| {
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
        let refs = s.unlinked_references(&id_of(&s, "Alpha")).unwrap();
        assert_eq!(titles(&refs), ["Beta", "Delta", "Gamma"]);
        let text = |r: &Reference| r.blocks.iter().map(|b| b.text.clone()).collect::<Vec<_>>();
        assert_eq!(text(&refs[0]), ["alpha is mentioned"]);
        assert_eq!(text(&refs[1]), ["omega here, via an alias"]);
        assert_eq!(text(&refs[2]), ["ALPHA, again!"], "not the type:: property");
    }

    #[test]
    fn references_list_journals_first_and_newest_first() {
        use jiff::ToSpan;
        let today = Zoned::now().date();
        let (_dir, s) = open_with(|g| {
            let index = Index::build(g).unwrap();
            g.create_entity("Alpha", &[]).unwrap();
            let zed = g.create_entity("Zed", &[]).unwrap();
            note(g, zed, "[[Alpha]]");
            for days in [3, 1, 2] {
                let day = ensure_journal(g, &index, today.checked_sub(days.days()).unwrap());
                note(g, day.unwrap(), "[[Alpha]]");
            }
        });
        let refs = s.references(&id_of(&s, "Alpha")).unwrap();
        let day = |n: i64| today.checked_sub(n.days()).unwrap().to_string();
        assert_eq!(titles(&refs), [day(1), day(2), day(3), "Zed".to_owned()]);
    }

    #[test]
    fn journals_have_no_unlinked_references() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::open(dir.path()).unwrap();
        let today = s.journals().unwrap().remove(0);
        assert!(s.unlinked_references(&today.id).unwrap().is_empty());
    }
}
