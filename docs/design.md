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

Repository layout (Cargo workspace now; pnpm workspace once there is UI):

```
crates/syntax    block Markdown syntax: properties, links, block refs
crates/core      graph model, index, journals, text import/export
crates/node      headless sync node binary (next)
crates/cli       import/export/fsck tooling (later)
apps/app         Tauri 2 shell (later)
apps/ui          Next.js UI, static export (later)
docs/
```

The Markdown syntax is needed on both sides: Rust for indexing, import and
export, and TypeScript for the editor. `crates/syntax` is the reference
implementation. The UI will use it compiled to WASM rather than keeping a
second parser in step.

## Data model

### Blocks

The block is the unit of everything. Each block has:

| Field  | Storage                     | Notes                                     |
|--------|-----------------------------|-------------------------------------------|
| id     | `LoroTree` node ID          | stable and unique, but internal to a graph |
| parent | the tree structure          | none for roots                            |
| order  | the tree's fractional index | concurrent moves can't create cycles      |
| text   | `LoroText` holding Markdown | merges character by character             |

All blocks live in one `LoroTree` inside one Loro document per graph. A move
between entities is then just a tree move, and Loro handles concurrent moves
for us, cycles included.

**The Markdown text is the source of truth for properties.** Nothing about a
block's properties is stored outside its text. Properties, links and
backlinks are all derived by parsing, and that derived state lives in an
index that can be rebuilt at any time and is never synced. The upshots:

- Export and import are lossless with no special cases.
- A property edit is a text edit, so it merges character by character like
  any other. Two devices changing the same value concurrently can therefore
  produce a merged string rather than one winner. With one user on several
  devices that's rare, and the result is visible and fixable, rather than
  silently dropping one side.
- The schema can't reject anything. Values that don't fit it are flagged.

### Properties

Properties are written in two ways, both parsed from the text:

- **Property lines**, as in Logseq's original format: `key:: value` on a line
  of its own. A key starts with a letter, digit or underscore, may contain
  spaces, and may not contain `:`, `#`, brackets or parentheses.
- **Inline properties**, anywhere in running text: `#key` or `#key(value)`.

A block's **type** is the value of its `type` property.

The two shapes of multiple values are distinguished explicitly:

| Written                          | Meaning                                   |
|----------------------------------|-------------------------------------------|
| `#tag(a) #tag(b)`, or two `tag::` lines | two instances of `tag`, one value each |
| `#steps([a, b, c])`, `steps:: [a, b, c]` | one instance holding an ordered list |

Lists are always written with brackets, and don't nest. In a property line a
comma means nothing special: `note:: a, b` is the text "a, b".

**Property names are unique.** A property key is an entity of type
`property` whose name is the key. If two devices create the same property
entity concurrently, the duplicates are merged like journals (below). Each
property entity can describe its values:

| Schema property | Values                                                       |
|-----------------|--------------------------------------------------------------|
| `value-type`    | `text`, `number`, `boolean`, `date`, `datetime`, `ref` (more later, e.g. `blob`) |
| `form`          | `scalar` or `list`                                           |
| `cardinality`   | `one` or `many`                                              |

A property with no entity, or one whose entity doesn't say, is untyped text,
scalar, any number of instances. Quiddity's own properties (`type`, `alias`,
`date`, `id`, `title`, and the three above) have built-in schemas, so no
bootstrap blocks or genesis snapshot are needed. Because names are unique,
a built-in is found by its name alone. A property entity for one of them is
optional and only adds documentation.

### Values

At the syntax level a value is one of:

- **text**: written bare, or `"quoted"` when it contains delimiters
- **a link**: `[[Name#qualifier(value)]]`
- **a block reference**: `((id))`
- **a list** of the above: `[a, "b, c", [[D]]]`

The property's schema then reads it as text, a number (an IEEE-754 double), a
boolean (`true`/`false`/`yes`/`no`), a date (ISO 8601 calendar date), a
datetime (RFC 3339, offset required) or a reference. For a `ref` property,
bare text is a name, so `type:: person` means `type:: [[person]]`.

Escapes: `\` before any of `\ ( ) [ ] , " #` makes it literal. Any other
backslash is literal, so `path:: C:\Users` reads as written. Quoted strings
also accept `\n` for a newline.

### Entities

An entity is the equivalent of a Logseq or Roam page: **a root block**. Its
text is its name and its children are the page body. If the first child
consists only of `key:: value` lines, it is the entity's **properties block**
and holds the entity's own properties. This mirrors the properties block at
the top of a Logseq page.

```
Jane                         ← root block: the name
├─ type:: person             ← properties block
│  born:: 1970-01-01
├─ Met at [[Summit]]         ← body
│  └─ …
```

- **Names are not unique.** An entity is identified by its name together with
  its properties.
- **Renaming** keeps the old name as an `alias` value, so links written
  against the old name (including by a device that was offline) still resolve.
- **Duplicates.** Some entities are fully identified by their properties:
  journals (type and date) and property keys (type and name). When two
  devices create the same one concurrently, the copy with the lowest ID is
  kept and the content of the others moves into it. The others are **not
  deleted**, because a device that hasn't yet synced may still be adding to
  its copy, and anything added under a deleted block is lost. Instead the
  index hides them, and the merge runs after every sync, moving anything new
  across. Every device makes the same moves, so concurrent merges converge.
  For other entities with the same name, the UI offers to merge.

### Journals and dates

A journal page is an entity named for its date, with a properties block of
`type:: journal` and `date:: 2026-10-25`.

### Block text and link syntax

| Syntax                                     | Meaning                                          |
|--------------------------------------------|--------------------------------------------------|
| `key:: value`                              | a property, on its own line                      |
| `#tag`                                     | property `tag` with no value (a plain tag)       |
| `#prop(value)`                             | one instance of `prop` with `value`              |
| `#prop([a, b])`                            | one instance holding a list                      |
| `#[[Multi word]](value)`                   | a property whose name contains spaces            |
| `[[Name]]`                                 | link to an entity called `Name`                  |
| `[[Name#type(person)#born(1970-01-01)]]`   | link narrowed by the target's properties         |
| `#author([[Jane#type(person)]])`           | a reference value, by name                       |
| `((id))`                                   | reference to the block with that `id` property   |

A qualifier `#key(value)` in a link matches an entity with an instance of
`key` whose value is equal to `value` (or contains it, for a list). A bare
`#key` matches an entity with any instance of `key`. Names, and values being
compared, ignore case and repeated whitespace.

Links in text are **resolved when followed**, not when written:

- Exactly one match: go to it.
- Several matches: show a disambiguation page listing the candidates and the
  properties that tell them apart, with an option to narrow the link in place.
- No match: offer to create the entity, taking its initial properties from the
  link.

The index records what each link resolves to, so backlinks include ambiguous
links (marked as such).

**Block references.** Block IDs are internal to a graph and don't survive
export and import. A block that something refers to gets an `id:: <uuid>`
property line, added when it's first referred to (as Logseq does), and
`((uuid))` resolves through that property.

Lexing rules:

- `#` starts a property only at the start of a line, after whitespace or
  `(`, or straight after another inline property (so `#a(1)#b(2)` is two).
  `# Heading`, `foo#bar` and URL fragments are not properties.
- All-digit keys (`#123`) are not properties, so issue numbers stay as text.
  `#fff` is a property; we can live with that.
- A trailing full stop isn't part of a bare key: `see #todo.` is `todo`.
- Code spans and fenced code blocks are opaque.
- Inside a link, `#` followed by a key starts a qualifier. Write `\#` for a
  literal one. `[[C# notes]]` is fine as it is.
- Anything malformed is plain text. Parsing never fails.

### Binaries

A binary is referenced from text by content hash, in a property whose
`value-type` is `blob` (a value type still to come). The value is a BLAKE3
hash, with the MIME type, size and an optional file name alongside. The bytes
live in the iroh-blobs store, not in the Loro document.

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
  quiddity.json            marks a Quiddity export, with its format version
  entities/
    Jane.md
    Jane 2.md              same name, different entity: file name suffix only
  journals/
    2026-10-25.md
  assets/                  (with blobs) <blake3>.<ext>
```

Each file is an outline in the style of Logseq's original format:

```markdown
title:: Jane
type:: person
born:: 1970-01-01

- Met at [[Platform Summit#type(event)]] #status(done)
  a second line of the same block
	- Child block with a ((0198f0c4-…)) reference
- Quotable
  id:: 0198f0c4-…
```

- The lines before the first bullet are the entity's name (`title::`, so
  names needn't be valid or unique file names) followed by its properties
  block.
- Child bullets are indented with one tab per level. Continuation lines get
  the bullet's tabs plus two spaces.
- Since text is the source of truth, block properties need no special
  handling: they're simply part of each block's text.
- Re-importing an export is lossless (tested as a round trip). With the
  manifest present, indentation is read strictly. Without it, as for a
  Logseq graph, it's judged by width, fenced code is respected, `logseq/` and
  hidden directories are skipped, and files in `journals/` named
  `2026_10_25.md` or `2026-10-25.md` become journal pages.
- Exports skip duplicates that have been fully merged. Importing the same
  export twice creates duplicates of everything except journals and property
  entities, which merge.

## UI model

- **Outliner**: every bullet is a block. A block shows rendered Markdown and
  becomes a CodeMirror 6 editor bound to its `LoroText` when focused, much like
  Logseq's edit-raw/view-rendered toggle.
- **Pages** are entities. **Journals** are the default home: today's page plus
  scroll-back.
- **Properties** are rendered as chips when a block isn't being edited, and
  as their Markdown when it is. The schema drives the editing aids (date
  picker, reference autocomplete, etc), which write Markdown into the text.
- **Linked and unlinked references** appear at the foot of every entity page.
  Disambiguation pages are as described above.
- **Presence**: other devices or users editing the same block appear as
  cursors, via the presence ALPN.
- Later: queries over properties, table and board views by type, and a
  plugin API.

## Decisions so far

- Properties: Logseq-style `key:: value` lines, with an entity's own
  properties in its initial block, plus inline `#prop(value)`. The Markdown is
  the source of truth.
- Property names are unique.
- Lists are explicit: `#prop([a, b])`.
- `crates/syntax` (Rust) is the reference parser. The UI uses it via WASM.

## Open questions

1. **Unsynced edits on eviction.** What happens to edits made on a device that
   was evicted: diff and re-apply, or kept as a separate entity to sort out by
   hand?
2. **At-rest encryption on the headless node.** Is the user's own node trusted
   with plaintext, or should it eventually run "blind" and store only encrypted
   snapshots?
3. **Concurrent edits to one property value.** Since properties are text, two
   devices editing the same value offline get a character-level merge. Is that
   acceptable, or should the UI detect a value that changed on both sides and
   ask?
