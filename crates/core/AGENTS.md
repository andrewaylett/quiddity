# quiddity-core

The graph on Loro, the index, schemas, journals, duplicate merging, and the
outline text format.

## Loro

- **Export commits pending edits.** `updates_since` and `snapshot` include
  uncommitted edits, so `Graph::commit` only marks a transaction boundary.
  Nothing observes that until undo exists, which is why its no-op mutant is
  excluded.
- **`version()` counts uncommitted edits too.**
- **Imports wait for their causes.** Updates whose earlier changes a replica
  hasn't seen are held pending without error. Exporting from a version the
  receiver doesn't have (say, from a fresh `Graph::new()` to a
  `Graph::blank()`) appears to import nothing. Test with replicas that share
  history, or send everything.
- `Graph::blank()` has no schema version until it syncs; `Graph::new()`
  writes one.

## The model

- **Markdown is the source of truth.** Properties, links and block refs are
  parsed from block text. The `Index` is derived, rebuilt from the graph,
  and never synced.
- An entity is a root block; its text is its name. Its first child, if it
  holds only property lines, is its properties block.
- **`move_block`'s index is the final position**, so within the same parent
  it counts the siblings other than the block itself.
- **Duplicates are never deleted.** Entities with the same identity
  (journals by date, property entities by name) are merged into the one
  with the lowest ID by moving their content. A device that hasn't synced
  may still be writing to a duplicate, and anything under a deleted block
  would be lost. `merge_duplicates` runs after every sync and reports
  whether it moved anything. Spent duplicates are hidden and left out of
  exports.

## The outline format

- **`Indent::Tabs` is Quiddity's own format and is lossless.** It splits
  lines on `\n` only, so a `\r` in a block's text survives. `Indent::Auto`
  reads other tools' files (Logseq): it uses `lines()`, nests by width and
  respects code fences.
- Continuation lines are written as the bullet's tabs plus two spaces.
- Logseq sometimes writes page properties as the first bullet; that block
  is lifted into the outline's properties, splitting on `\n` only.

## Tests

`tests/common` has the shared generators (`text()`, `op()`) and replica
helpers. `common::config(n)` reads `PROPTEST_CASES`, so `n` is only the
default.
