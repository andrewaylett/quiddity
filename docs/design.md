# Quiddity: initial design

Status: draft, for discussion. Nothing here is implemented yet.

Quiddity is a local-first second brain in the vein of Logseq, Obsidian and
Roam. It should feel familiar to people who have used those tools, run
primarily as a desktop and mobile app, sync between a user's devices without a
central service, export to and import from a sensible text format, and store
typed binaries.

## Goals

- **Local first.** Every device holds the whole graph and works fully offline.
- **Familiar.** Outliner editing, pages, journals, `[[links]]`, `#tags`,
  backlinks.
- **Peer-to-peer sync.** No central service. A headless node the user runs
  themselves fills in when portable devices aren't online at the same time.
- **Text in, text out.** A Markdown export that is readable on its own and good
  enough to re-import as a backup.
- **Extensible.** Types and properties are data, not code. New value types and
  block types can be added without breaking stored graphs.

## Non-goals (for now)

- A hosted web version. Next.js is purely the UI shell.
- Sharing part of a graph. Sharing is whole-graph only.
- Large-scale collaboration. The main case is one user with several devices.
  Multiple users on one graph should work, but it isn't what we're optimising
  for.
- Older app versions reading data written by newer ones (see
  [Versioning](#versioning-and-migration)).

## Architecture

```
┌──────────────── Tauri 2 app (desktop + iOS + Android) ────────────────┐
│                                                                        │
│  UI: Next.js (static export) + TypeScript                              │
│   ├─ editor bound to a Loro replica (loro-crdt, WASM)                  │
│   └─ talks to the core over Tauri IPC (Loro updates + queries)         │
│                                                                        │
│  Core: Rust                                                            │
│   ├─ Loro document (one per graph): source of truth                    │
│   ├─ SQLite: persisted snapshot + update log, derived indexes, FTS     │
│   ├─ blob store: content-addressed binaries (iroh-blobs, BLAKE3)       │
│   └─ iroh endpoint: sync, presence and blob transfer between devices   │
└────────────────────────────────────────────────────────────────────────┘

┌──────── quiddity-node (headless) ────────┐
│  the same Rust core, without the UI;     │
│  always online, keeps full history       │
│  and all blobs                           │
└──────────────────────────────────────────┘
```

- **Tauri 2** for every platform. Networking, storage and indexing run in
  native Rust. A WebView can't reliably listen for connections or do local
  network discovery, and iOS evicts browser storage when it likes.
- **The UI holds its own Loro replica.** It exchanges updates with the core over
  IPC, using the same protocol peers use with each other. Keystrokes are applied
  locally in the UI, so typing never waits on IPC.
- **The headless node is the core run as a daemon.** It is a full peer owned by
  the user, not a dumb relay. That lets it keep full history and serve devices
  that have been offline for a long time.

Suggested repository layout (pnpm workspace plus Cargo workspace):

```
crates/core      graph model, storage, sync protocol, import/export
crates/node      headless sync node binary
crates/cli       import/export/fsck tooling (later)
apps/app         Tauri 2 shell
apps/ui          Next.js UI (static export)
packages/model   TypeScript model types + Markdown parser/serialiser
docs/
```

The Markdown parser and serialiser are needed on both sides: TypeScript for the
editor, Rust for import, export and indexing. One of them has to be the
reference implementation, tested against a shared corpus of examples.

## Data model

### Blocks

The block is the unit of everything. Each block has:

| Field      | Storage                                 | Notes                                      |
|------------|-----------------------------------------|--------------------------------------------|
| id         | `LoroTree` node ID                      | stable, unique, the primary key            |
| parent     | the tree structure                      | `null` for roots                           |
| order      | the tree's fractional index             | concurrent moves can't create cycles       |
| text       | `LoroText` holding Markdown             | merges character by character              |
| properties | `LoroMap` (see below)                   | typed and schema-described                 |

All blocks live in one `LoroTree` inside one Loro document per graph. A move
between entities is then just a tree move, and Loro handles concurrent moves
for us, cycles included.

A block's **type** is the value of the well-known `type` property.

### The meta-schema (bootstrap)

Property keys are blocks, and so are types. A handful of blocks have fixed,
well-known IDs and are created identically on every device. That way two
devices bootstrapping a graph converge instead of creating duplicates:

- `type`: the property that gives a block's type
- `name`: the property that gives a display name
- `property`: the type of property-key blocks
- `entity`, `journal`, `class`, …: built-in types
- the schema properties used by property-key blocks: `value-type`, `form`,
  `cardinality`, `alias`, …

Since a Loro tree node ID can't be chosen up front, these blocks are created
from a fixed **genesis snapshot** that ships with the app. Every graph starts
from that same snapshot.

### Properties

A block's properties are a `LoroMap` from property-key block ID to a list of
**instances**:

```
properties: LoroMap<keyId, LoroList<Instance>>
Instance:   { id, value: Value }            // scalar form
          | { id, items: LoroMovableList<Value> }  // list form
```

That gives us the two shapes you asked for:

- **Many instances** of a property: several entries in the instance list. Each
  has its own ID, so concurrent additions both survive.
- **One instance holding an ordered list**: a single instance in list form. Its
  items are a movable list, so they can be reordered concurrently.

Each property-key block (type `property`) carries its own schema:

| Schema property | Values                                                     |
|-----------------|------------------------------------------------------------|
| `value-type`    | `string`, `number`, `boolean`, `date`, `datetime`, `ref`, `blob`, … |
| `form`          | `scalar` or `list`                                         |
| `cardinality`   | `one` or `many`                                            |
| `ref-type`      | for `ref`: restricts which types a reference may point at  |

The schema is advisory. Data that doesn't match it, whether from concurrent
edits, imports or schema changes, is kept and flagged in the UI, never
silently dropped. A property with cardinality `one` that ends up with two
instances (from two devices editing concurrently) shows the instance with the
lowest ID and offers a choice.

### Values

Values are tagged so that new types can be added later:

```
{ t: "str",  v: string }
{ t: "num",  v: number }            // IEEE-754 double; `dec` (decimal string) later if needed
{ t: "bool", v: boolean }
{ t: "date", v: "2026-10-25" }      // ISO 8601 calendar date, no time zone
{ t: "dt",   v: "2026-10-25T21:04:00+01:00" }  // RFC 3339, offset kept
{ t: "ref",  v: blockId }
{ t: "blob", v: { hash, mime, size, name? } }
```

A client that meets a tag it doesn't know keeps the value and passes it along
as it is, showing it as an opaque value rather than discarding it.

References in property values are stored by **ID**. References in **text** are
by name (below). On export, ID references are written in the textual form.

### Entities

An entity is the equivalent of a Logseq or Roam page: a block whose `type`
includes `entity`, or a subtype of it. Its children are the page body.

- **Names are not unique.** An entity is identified by its name together with
  its other properties.
- **Renaming** keeps the old name as an `alias` instance, so links written
  against the old name (including by a device that was offline) still resolve.
- **Duplicates.** If two devices concurrently create entities whose
  identifying properties are equal, both survive replication. For types whose
  identity is fully determined by their properties, such as journals, we merge
  deterministically: the lowest ID wins, the loser's children move under the
  winner, its properties are unioned and the loser is deleted. Every device
  applies the same rule, so concurrent merges converge. For anything else the
  UI offers the merge.

### Journals and dates

A journal page is an entity of type `journal` with a single `date` property:
`#type(journal)#date(2026-10-25)`. Type plus date identifies it fully, so
journal duplicates are merged automatically as described above.

### Block text and link syntax

Block text is canonically **Markdown**, stored in a `LoroText` and merged
character by character.

Properties are written `#prop(value)`. Following from that:

| Syntax                                     | Meaning                                                 |
|--------------------------------------------|---------------------------------------------------------|
| `#tag`                                     | property `tag` present with no value (a plain tag)       |
| `#prop(value)`                             | one instance of `prop` with `value`                     |
| `#prop(a)#prop(b)`                         | two instances                                           |
| `#[[Multi word]](value)`                   | a property key whose name contains spaces                |
| `[[Name]]`                                 | link to the entity called `Name`                         |
| `[[Name#type(person)#born(1970-01-01)]]`   | link narrowed by properties                              |
| `#author([[Jane#type(person)]])`           | a reference value written by name                        |
| `((blockId))`                              | reference to a non-entity block by ID                    |

Links in text are **resolved when followed**, not when written:

- Exactly one match: go to it.
- Several matches: show a disambiguation page listing the candidates and the
  properties that tell them apart, with an option to narrow the link in place.
- No match: offer to create the entity, taking its initial properties from the
  link.

The backlink index stores what each link resolves to and is re-resolved
whenever a name, alias or identifying property changes.

Lexing rules to settle:

- `#` must begin a word (`# Heading`, `foo#bar` and URL fragments are not
  tags). Code spans and code blocks are opaque.
- Should all-digit tags (`#123`) and hex colours (`#fff`) be tags? Both Logseq
  and Obsidian have rules for this; we should copy whichever is least
  surprising.
- Escaping: `\)` and `\#` inside values. Strings may also be quoted (`"…"`).

### Binaries

A binary is a `blob` value: a BLAKE3 hash plus MIME type, size and an optional
file name. The bytes live in the iroh-blobs store, not in the Loro document.

- Portable devices fetch blobs **when they're opened** and may evict them
  later. The headless node keeps them all.
- An `asset` block type holds a blob together with metadata such as captions
  and dimensions, so that embedding is just a block reference.
- Garbage collection: unreferenced blobs are deleted after a grace period, and
  only once every device's acknowledged version is past the point where the
  last reference was removed.

## Storage

- The **Loro document** is the only source of truth. It is persisted in SQLite
  as the latest snapshot plus an append-only log of updates, compacted
  periodically.
- **Derived indexes** live in SQLite tables, can be rebuilt from the document
  at any time, and are never synced:
  - name and alias → block IDs
  - property key and value → block IDs
  - links and backlinks, with how each resolved
  - full-text search (FTS5)
- **Blobs** live on disk in the iroh-blobs store.

## Sync

### Transport: iroh

- Each device is an iroh endpoint and is identified by its node ID (an Ed25519
  public key).
- Connections are authenticated QUIC. They use hole punching where possible and
  fall back to a relay; we can run our own relays.
- Protocols are separated by ALPN:
  - `quiddity/sync/1`: Loro version-vector exchange and updates
  - `quiddity/presence/1`: ephemeral state such as cursors, selections and
    "currently editing"
  - iroh-blobs ALPN: binaries

### Devices and pairing

- The list of authorised devices is stored **in the graph**, as blocks of type
  `device` holding the node ID, a name and a `revoked` flag. Adding or revoking
  a device is an ordinary edit that syncs like everything else.
- Pairing: an existing device shows a QR code with its node ID and a one-time
  token. The new device connects, presents the token and is added.
- A peer accepts connections only from node IDs listed and not revoked in its
  own copy of the graph.

### Protocol

1. On connecting, peers exchange their app and schema versions and their Loro
   version vectors.
2. Each sends the updates the other is missing (`export({ mode: "update", from })`).
3. While connected, local updates are pushed as they happen, so live editing
   syncs character by character.
4. Each peer periodically sends an **acknowledgement**: the version vector it
   has durably stored.

### Live edits versus stored history

These follow your original read-marker idea, but the markers live in sync
metadata rather than inside the documents being edited:

- Every update is sent live at full detail to whichever peers are online.
- Each device records, for every peer, the most recent acknowledgement it has
  received from that peer.
- **The headless node keeps full history** for at least as long as it is
  needed to catch up the slowest device it still knows about.
- **Portable devices keep shallow history.** They periodically replace their
  stored document with a Loro shallow snapshot taken at the earlier of two
  points:
  - the version the headless node has acknowledged, since history before that
    can always be fetched from the node again;
  - the start of the local undo stack.
- **Eviction.** A device that hasn't acknowledged anything for N days
  (configurable; 30 by default) stops holding back the node's compaction. When
  it next connects it re-syncs from a snapshot. Its unsynced local edits are
  saved as a diff against the fresh snapshot and offered for re-application.
  Exactly how is an open question.

### Undo

Undo uses Loro's `UndoManager` on each device and covers that device's own
edits only. The stack is kept in memory and limited in depth, which also sets
the local compaction horizon. A multi-block operation (indent, move, paste a
tree) is a single Loro transaction, so it syncs and undoes as one unit. Because
it's one document, there are no non-atomic edits across documents to worry
about.

## Versioning and migration

- The document carries a `schemaVersion`.
- **New clients read old data.** Migration only goes forwards. It is done **on
  read** through version lenses, and writes always use the current format. We
  don't rewrite the document wholesale: two devices running the same
  migration concurrently would produce duplicate edits.
- When a migration must change stored data (rare, e.g. restructuring the
  meta-schema), it is written as a deterministic, idempotent transform keyed by
  migration ID and recorded in the document, so a second run does nothing.
- **Older clients don't read newer data.** The sync handshake compares schema
  versions. A device that is behind keeps working on its own data and stays
  read-only for synced data until it is upgraded. It never imports updates it
  can't understand.
- **Unknown value tags and unknown properties are always kept**, so that within
  one schema version newer value types can be introduced without breaking
  anything.

## Text export and import

Export is a directory:

```
quiddity-export/
  quiddity.json            format version, graph ID, export time
  schema/                  property and type definitions as entities
  entities/
    Jane (person).md
    Jane (person) 2.md     same name and properties: filename suffix only
  journals/
    2026-10-25.md
  assets/
    <blake3>.<ext>
```

Each file is a Markdown outline:

```markdown
#type(person)#born(1970-01-01)

- Met at [[Platform Summit#type(event)]]
  #status(done)
  - Child block with a ((0198f0c4-…)) reference
- Photo #file(assets/9f2c….jpg)
  #id(0198f0c4-…)
```

- The page's own properties go on the first line.
- Block properties go on a continuation line under the block.
- `#id(…)` is written only for blocks that something references by ID
  (`((…))`), as Logseq does with `id::`.
- Import is lossless for anything that came from an export. Plain Logseq or
  Obsidian Markdown imports on a best-effort basis: `key:: value` and YAML
  front matter become properties.

## UI model

- **Outliner**: every bullet is a block. A block shows rendered Markdown and
  becomes a CodeMirror 6 editor bound to its `LoroText` when focused, much like
  Logseq's edit-raw/view-rendered toggle.
- **Pages** are entities. **Journals** are the default home: today's page plus
  scroll-back.
- **Properties** appear as chips. Typing `#prop(value)` creates one; the schema
  drives the value editor (date picker, reference autocomplete, etc).
- **Linked and unlinked references** appear at the foot of every entity page.
  Disambiguation pages are as described above.
- **Presence**: other devices or users editing the same block appear as
  cursors, via the presence ALPN.
- Later: queries over properties, table and board views by type, and a
  plugin API.

## Open questions

1. **Where properties live relative to text.** This draft stores properties
   structurally and treats `#prop(value)` as the way they are typed and
   exported. If a sentence contains `#prop(value)` midway, do we keep the token
   in the text, so the text stays the reference and the property is derived
   from it, or lift it out into a chip? Keeping it in the text gives perfect
   round-tripping but makes the text the source of truth for properties.
   Lifting it out keeps typed, last-writer-wins semantics.
2. **Uniqueness of property names.** Inside `[[Name#type(person)]]`, `type` is
   itself resolved by name. Should property-key names be unique within a graph,
   with duplicates merged like journals, or do property keys need
   disambiguating too, recursively?
3. **List syntax.** How is a single instance holding a list written?
   `#prop(a, b)` driven by the schema, or explicit `#prop([a, b])` so it can be
   parsed without knowing the schema?
4. **Unsynced edits on eviction.** What happens to edits made on a device that
   was evicted: diff and re-apply, or kept as a separate entity to sort out by
   hand?
5. **At-rest encryption on the headless node.** Is the user's own node trusted
   with plaintext, or should it eventually run "blind" and store only encrypted
   snapshots?
6. **Parser reference implementation.** Rust compiled to WASM shared with the
   UI, or TypeScript and Rust kept in step by a shared test corpus?
