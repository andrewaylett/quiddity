use std::borrow::Cow;
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

/// What a replica has seen: for each peer, how many of its edits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Version(VersionVector);

impl Version {
    pub fn encode(&self) -> Vec<u8> {
        self.0.encode()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Ok(Self(VersionVector::decode(bytes)?))
    }

    /// Extends this version to include everything in `other`.
    pub fn merge(&mut self, other: &Version) {
        self.0.merge(&other.0);
    }

    /// Whether everything in `other` is included in this version.
    pub fn includes(&self, other: &Version) -> bool {
        self.0.includes_vv(&other.0)
    }

    /// What both versions include.
    pub fn meet(&self, other: &Version) -> Version {
        Version(self.0.intersection(&other.0))
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
        // A blank replica stays truly blank: a Loro document that has
        // imported anything, even nothing, ignores a compacted snapshot.
        if Self::version_of_updates(bytes)? != Version::default() {
            graph.import(bytes)?;
        }
        Ok(graph)
    }

    /// The schema version the graph was created with, if known yet.
    pub fn schema_version(&self) -> Option<i64> {
        match self.doc.get_map(GRAPH).get(SCHEMA_VERSION_KEY) {
            Some(ValueOrContainer::Value(LoroValue::I64(v))) => Some(v),
            _ => None,
        }
    }

    /// Exports the whole graph, history included.
    pub fn snapshot(&self) -> Result<Vec<u8>> {
        Ok(self.doc.export(ExportMode::Snapshot)?)
    }

    /// Everything this replica has seen.
    pub fn version(&self) -> Version {
        Version(self.doc.oplog_vv())
    }

    /// The updates a replica at `version` is missing. A replica whose version
    /// predates this one's history can't be brought up to date with updates
    /// (they would never apply), so that is an error.
    pub fn updates_since(&self, version: &Version) -> Result<Vec<u8>> {
        if !self.can_update(version) {
            return Err(Error::BeforeHistory);
        }
        Ok(self.doc.export(ExportMode::updates(&version.0))?)
    }

    /// The version this replica's history starts at: empty if it has all of
    /// it, or where it was compacted by [`Graph::shallow_snapshot`].
    pub fn history_start(&self) -> Version {
        Version(self.doc.shallow_since_vv().to_vv())
    }

    /// Whether [`Graph::updates_since`] can bring a replica at `version` up
    /// to date.
    pub fn can_update(&self, version: &Version) -> bool {
        version.includes(&self.history_start())
    }

    /// A snapshot that keeps history only from `at` on (or from as close to
    /// it as this replica can). Loading it gives a replica that can only
    /// update peers that have reached `at`, and can only apply edits made on
    /// top of `at`.
    ///
    /// So it is safe to compact at a version every replica has reached once
    /// this replica has everything any replica has: whatever arrives later
    /// was made on top of `at`.
    pub fn shallow_snapshot(&self, at: &Version) -> Result<Vec<u8>> {
        let start = self.history_start();
        let mut at = at.meet(&self.version());
        at.merge(&start);
        if at == start {
            // Nothing more to drop.
            return self.snapshot();
        }
        let frontiers = self.doc.vv_to_frontiers(&at.0);
        Ok(self
            .doc
            .export(ExportMode::ShallowSnapshot(Cow::Owned(frontiers)))?)
    }

    /// The version a replica reaches by applying `updates` (as made by
    /// [`Graph::updates_since`]), as far as the updates themselves say.
    pub fn version_of_updates(updates: &[u8]) -> Result<Version> {
        Ok(Version(
            LoroDoc::decode_import_blob_meta(updates, false)?.partial_end_vv,
        ))
    }

    /// Applies updates or a snapshot from another replica.
    ///
    /// A compacted replica can't apply edits made on history it has dropped:
    /// that is [`Error::BeforeHistory`]. Anything else that can't be applied
    /// here (updates whose causes this replica hasn't seen, which Loro would
    /// hold back without saying) is [`Error::NotApplied`].
    pub fn import(&self, bytes: &[u8]) -> Result<()> {
        match self.doc.import(bytes) {
            Ok(_) => {}
            Err(loro::LoroError::ImportUpdatesThatDependsOnOutdatedVersion) => {
                return Err(Error::BeforeHistory);
            }
            Err(e) => return Err(e.into()),
        }
        if !self.version().includes(&Self::version_of_updates(bytes)?) {
            return Err(Error::NotApplied);
        }
        Ok(())
    }

    /// Ends the current transaction. Edits made since the last commit sync
    /// and undo as one unit.
    pub fn commit(&self) {
        self.doc.commit();
    }

    pub(crate) fn doc(&self) -> &LoroDoc {
        &self.doc
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

    /// Replaces `len` UTF-16 code units of a block's text, starting at `pos`,
    /// with `insert`: the unit CodeMirror and JavaScript strings count in, so
    /// an editor's changes apply as they were made rather than as a diff.
    pub fn splice_text(&self, id: BlockId, pos: usize, len: usize, insert: &str) -> Result<()> {
        let text = self.text_container(id)?;
        let end = pos.saturating_add(len);
        if end > text.len_utf16() {
            return Err(Error::OutOfRange(id));
        }
        text.splice_utf16(pos, len, insert)?;
        Ok(())
    }

    /// The length of a block's text in UTF-16 code units.
    pub fn text_len_utf16(&self, id: BlockId) -> Result<usize> {
        Ok(self.text_container(id)?.len_utf16())
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
    /// `index` among its new siblings or at the end. The index is the block's
    /// final position, so when moving within the same parent it counts the
    /// siblings other than the block itself.
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

    // Tree operations are checked against a model in tests/graph_model.rs,
    // and snapshots and updates in tests/sync.rs.

    #[test]
    fn deleted_blocks_are_gone() -> Result<()> {
        let graph = Graph::new();
        let page = graph.create_block(None, None, "Page")?;
        let child = graph.create_block(Some(page), None, "child")?;
        let grandchild = graph.create_block(Some(child), None, "grandchild")?;
        assert_eq!(graph.entity_of(grandchild), page);
        graph.delete_block(child)?;
        for gone in [child, grandchild] {
            assert!(!graph.contains(gone));
            assert!(matches!(graph.text(gone), Err(Error::NoSuchBlock(_))));
            assert!(graph.create_block(Some(gone), None, "x").is_err());
        }
        Ok(())
    }

    #[test]
    fn splicing_counts_utf16_units() -> Result<()> {
        let graph = Graph::new();
        let block = graph.create_block(None, None, "a😀b")?;
        // The emoji is two UTF-16 units, so `b` starts at 3.
        assert_eq!(graph.text_len_utf16(block)?, 4);
        graph.splice_text(block, 3, 1, "B!")?;
        graph.splice_text(block, 1, 2, "-")?;
        assert_eq!(graph.text(block)?, "a-B!");
        assert!(graph.splice_text(block, 3, 2, "").is_err(), "past the end");
        assert!(graph.splice_text(block, 9, 0, "x").is_err());
        assert_eq!(
            graph.text(block)?,
            "a-B!",
            "a rejected splice changes nothing"
        );
        Ok(())
    }

    #[test]
    fn block_ids_round_trip_as_text() -> Result<()> {
        let graph = Graph::new();
        let block = graph.create_block(None, None, "x")?;
        assert_eq!(block.to_string().parse::<BlockId>()?, block);
        Ok(())
    }
}
