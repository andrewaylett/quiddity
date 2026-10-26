# quiddity-view

What a UI shows of a graph (queries) and the edits it makes to it (the
outliner's). It owns a `Graph` and an `Index` over it, and returns plain data.
`quiddity-wasm` compiles it for the browser, where the UI holds its own copy
of the graph; nothing here knows about JavaScript or Tauri.

## The index

- **It is rebuilt lazily.** Edits mark the view stale (`changed`); the next
  query rebuilds. Anything done to the graph directly (an import, say) must
  be followed by `View::changed`, or queries answer from the old index.
- **Don't hold the index across a call that may rebuild it.** It sits in a
  `RefCell`, and a rebuild needs the borrow exclusively, so a `Ref` kept while
  another method runs panics once an edit has made it stale. Take it, use it,
  drop it; `block` shows the pattern.
- A rebuild reads every block, so an edit followed by a query costs time in
  proportion to the graph (about 18 ms at 10,000 blocks). The UI therefore
  asks for fresh answers after a pause in typing, not per keystroke.
- There is no clock. Anything that depends on the date (`journals`,
  `ensure_journal`) takes `today`.

## Wire format

The types in `query.rs` are what crosses to JavaScript and are mirrored by
`apps/ui/lib/model.ts`: change both together. IDs are strings. Names are
camelCase. **Offsets are UTF-16 code units** (`Property::start`/`end`, the
`pos` of `splice_text`, `split`), because that is what JavaScript strings and
CodeMirror count in; Rust's byte offsets never leave the crate. A missing
value is `None` here and `null` in JavaScript (the wasm layer sets that).

## Entities and blocks

- `entity()` accepts the ID of an entity that has been merged into another
  and answers for the one it was merged into, whose ID it reports. Callers
  should use the ID in the answer.
- **Block IDs don't survive an undo** of a deletion or redo of a creation
  (see `quiddity-core`). Tests compare text, not IDs, across an undo.
- Links are looked up in `Entity::links` by the text between their brackets as
  the syntax crate writes it (`link_key`). A link written some other way
  isn't found and renders as unresolved.

## Edits

- **Each edit is one commit, so one undo step** (a split is a splice and a
  create). Typing is the exception: `splice_text` opens an undo group, so a
  burst is one step, until `end_typing`.
- **Every other edit must call `structural()` first**, which ends any open
  burst. A new edit that forgets it would be merged into the typing before it.
- Edits that can't apply change nothing: `split` checks the offset is between
  characters before touching the text, and `merge_into_previous` says `None`
  rather than failing when there is nothing to merge into.
- Reading order is depth first; "the block before" a block is the previous one
  in that order, which for a first child is its parent.

## Tests

`tests.rs` builds graphs in memory with `view_with` and `page`. The queries,
the edits, properties and undo are separate modules. Offsets are checked
against text with emoji and accents, where bytes and UTF-16 units differ.
Mutation testing covers the whole crate and should stay clean.
