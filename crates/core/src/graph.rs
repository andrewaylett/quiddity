use std::fmt;
use std::str::FromStr;

use loro::{
    Container, ExportMode, LoroDoc, LoroText, LoroTree, LoroValue, TreeID, TreeParentId,
    UpdateOptions, ValueOrContainer, VersionVector,
};

use crate::{Error, Result};

const BLOCKS: &str = "blocks";
const GRAPH: &str = "graph";
const TEXT: &str = "text";
const SCHEMA_VERSION_KEY: &str = "schemaVersion";

/// The storage schema version written by this build.
pub const SCHEMA_VERSION: i64 = 1;

/// A block's identity: its node ID in the graph's tree.
///
/// This is internal to a graph and doesn't survive export and import; a block
/// that others refer to with `((…))` gets an `id` property instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(TreeID);

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for BlockId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self(TreeID::try_from(s)?))
    }
}

/// One replica of a graph.
#[derive(Debug)]
pub struct Graph {
    doc: LoroDoc,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Graph {
    /// Creates a new, empty graph.
    pub fn new() -> Self {
        let graph = Self::blank();
        graph
            .doc
            .get_map(GRAPH)
            .insert(SCHEMA_VERSION_KEY, SCHEMA_VERSION)
            .expect("inserting into a fresh map can't fail");
        graph.doc.commit();
        graph
    }

    /// Creates a replica with no content at all, for a device that is about
    /// to receive an existing graph by syncing.
    pub fn blank() -> Self {
        let doc = LoroDoc::new();
        doc.get_tree(BLOCKS).enable_fractional_index(0);
        Self { doc }
    }

    /// Loads a replica from a snapshot.
    pub fn from_snapshot(bytes: &[u8]) -> Result<Self> {
        let graph = Self::blank();
        graph.doc.import(bytes)?;
        Ok(graph)
    }

    /// The schema version the graph was created with, if known yet.
    pub fn schema_version(&self) -> Option<i64> {
        match self.doc.get_map(GRAPH).get(SCHEMA_VERSION_KEY) {
            Some(ValueOrContainer::Value(LoroValue::I64(v))) => Some(v),
            _ => None,
        }
    }

    /// This replica's Loro peer ID.
    pub fn peer_id(&self) -> u64 {
        self.doc.peer_id()
    }

    /// Exports the whole graph, history included.
    pub fn snapshot(&self) -> Result<Vec<u8>> {
        Ok(self.doc.export(ExportMode::Snapshot)?)
    }

    /// An encoded version vector describing everything this replica has seen.
    pub fn version(&self) -> Vec<u8> {
        self.doc.oplog_vv().encode()
    }

    /// The updates a peer at `version` (from [`Graph::version`]) is missing.
    pub fn updates_since(&self, version: &[u8]) -> Result<Vec<u8>> {
        let vv = VersionVector::decode(version)?;
        Ok(self.doc.export(ExportMode::updates(&vv))?)
    }

    /// Applies updates or a snapshot from another replica.
    pub fn import(&self, bytes: &[u8]) -> Result<()> {
        self.doc.import(bytes)?;
        Ok(())
    }

    /// Ends the current transaction. Edits made since the last commit sync
    /// and undo as one unit.
    pub fn commit(&self) {
        self.doc.commit();
    }

    fn tree(&self) -> LoroTree {
        self.doc.get_tree(BLOCKS)
    }

    /// The root blocks, which are the graph's entities, in order.
    pub fn roots(&self) -> Vec<BlockId> {
        self.children_of(TreeParentId::Root)
    }

    /// A block's children, in order.
    pub fn children(&self, id: BlockId) -> Vec<BlockId> {
        self.children_of(TreeParentId::Node(id.0))
    }

    fn children_of(&self, parent: TreeParentId) -> Vec<BlockId> {
        self.tree()
            .children(parent)
            .unwrap_or_default()
            .into_iter()
            .map(BlockId)
            .collect()
    }

    /// A block's parent, or `None` for a root block.
    pub fn parent(&self, id: BlockId) -> Option<BlockId> {
        match self.tree().parent(id.0) {
            Some(TreeParentId::Node(p)) => Some(BlockId(p)),
            _ => None,
        }
    }

    /// Whether the block exists and hasn't been deleted.
    pub fn contains(&self, id: BlockId) -> bool {
        let tree = self.tree();
        tree.contains(id.0) && !tree.is_node_deleted(&id.0).unwrap_or(true)
    }

    /// Every live block, depth first, in document order.
    pub fn blocks(&self) -> Vec<BlockId> {
        let mut out = Vec::new();
        let mut stack: Vec<BlockId> = self.roots().into_iter().rev().collect();
        while let Some(id) = stack.pop() {
            out.push(id);
            stack.extend(self.children(id).into_iter().rev());
        }
        out
    }

    /// The root block above `id`, or `id` itself if it is a root.
    pub fn entity_of(&self, mut id: BlockId) -> BlockId {
        while let Some(parent) = self.parent(id) {
            id = parent;
        }
        id
    }

    fn check(&self, id: BlockId) -> Result<()> {
        if self.contains(id) {
            Ok(())
        } else {
            Err(Error::NoSuchBlock(id))
        }
    }

    fn text_container(&self, id: BlockId) -> Result<LoroText> {
        self.check(id)?;
        match self.tree().get_meta(id.0)?.get(TEXT) {
            Some(ValueOrContainer::Container(Container::Text(text))) => Ok(text),
            _ => Err(Error::MissingText(id)),
        }
    }

    /// A block's Markdown text.
    pub fn text(&self, id: BlockId) -> Result<String> {
        Ok(self.text_container(id)?.to_string())
    }

    /// Replaces a block's text, recorded as a minimal character-level diff so
    /// that it merges cleanly with concurrent edits.
    pub fn set_text(&self, id: BlockId, text: &str) -> Result<()> {
        self.text_container(id)?
            .update(text, UpdateOptions::default())?;
        Ok(())
    }

    /// Creates a block under `parent` (or as a new root), at `index` among its
    /// siblings or at the end.
    pub fn create_block(
        &self,
        parent: Option<BlockId>,
        index: Option<usize>,
        text: &str,
    ) -> Result<BlockId> {
        if let Some(p) = parent {
            self.check(p)?;
        }
        let tree = self.tree();
        let parent = parent_id(parent);
        let node = match index {
            Some(i) => tree.create_at(parent, i)?,
            None => tree.create(parent)?,
        };
        let meta = tree.get_meta(node)?;
        meta.insert_container(TEXT, LoroText::new())?
            .insert(0, text)?;
        Ok(BlockId(node))
    }

    /// Moves a block, with its children, under `parent` (or to the roots), at
    /// `index` among its new siblings or at the end.
    pub fn move_block(
        &self,
        id: BlockId,
        parent: Option<BlockId>,
        index: Option<usize>,
    ) -> Result<()> {
        self.check(id)?;
        let tree = self.tree();
        let parent = parent_id(parent);
        match index {
            Some(i) => tree.mov_to(id.0, parent, i)?,
            None => tree.mov(id.0, parent)?,
        }
        Ok(())
    }

    /// Deletes a block and its children.
    pub fn delete_block(&self, id: BlockId) -> Result<()> {
        self.check(id)?;
        self.tree().delete(id.0)?;
        Ok(())
    }
}

fn parent_id(parent: Option<BlockId>) -> TreeParentId {
    parent.map_or(TreeParentId::Root, |p| TreeParentId::Node(p.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_read_blocks() -> Result<()> {
        let graph = Graph::new();
        assert_eq!(graph.schema_version(), Some(SCHEMA_VERSION));
        let page = graph.create_block(None, None, "Page")?;
        let a = graph.create_block(Some(page), None, "a")?;
        let c = graph.create_block(Some(page), None, "c")?;
        let b = graph.create_block(Some(page), Some(1), "b")?;
        let a1 = graph.create_block(Some(a), None, "a1")?;
        assert_eq!(graph.roots(), vec![page]);
        assert_eq!(graph.children(page), vec![a, b, c]);
        assert_eq!(graph.blocks(), vec![page, a, a1, b, c]);
        assert_eq!(graph.parent(a1), Some(a));
        assert_eq!(graph.entity_of(a1), page);
        assert_eq!(graph.text(b)?, "b");
        Ok(())
    }

    #[test]
    fn move_and_delete() -> Result<()> {
        let graph = Graph::new();
        let page = graph.create_block(None, None, "Page")?;
        let a = graph.create_block(Some(page), None, "a")?;
        let b = graph.create_block(Some(page), None, "b")?;
        graph.move_block(b, Some(page), Some(0))?;
        assert_eq!(graph.children(page), vec![b, a]);
        graph.move_block(b, Some(a), None)?;
        assert_eq!(graph.children(a), vec![b]);
        assert!(
            graph.move_block(a, Some(b), None).is_err(),
            "cycles are refused"
        );
        graph.delete_block(a)?;
        assert!(!graph.contains(a) && !graph.contains(b));
        assert_eq!(graph.blocks(), vec![page]);
        assert!(matches!(graph.text(a), Err(Error::NoSuchBlock(_))));
        Ok(())
    }

    #[test]
    fn text_edits() -> Result<()> {
        let graph = Graph::new();
        let block = graph.create_block(None, None, "hello world")?;
        graph.set_text(block, "hello there, world")?;
        assert_eq!(graph.text(block)?, "hello there, world");
        Ok(())
    }

    #[test]
    fn snapshot_round_trip() -> Result<()> {
        let graph = Graph::new();
        let page = graph.create_block(None, None, "Page")?;
        graph.create_block(Some(page), None, "child")?;
        let copy = Graph::from_snapshot(&graph.snapshot()?)?;
        assert_eq!(copy.schema_version(), Some(SCHEMA_VERSION));
        assert_eq!(copy.blocks(), graph.blocks());
        Ok(())
    }

    #[test]
    fn block_id_round_trip() -> Result<()> {
        let graph = Graph::new();
        let block = graph.create_block(None, None, "x")?;
        assert_eq!(block.to_string().parse::<BlockId>()?, block);
        Ok(())
    }
}
