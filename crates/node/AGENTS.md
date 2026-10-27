# quiddity-node

Sync over iroh (`Peer`), the on-disk `Store`, and the `quiddity-node` CLI.

- **A `Peer` relays.** Updates from one connection call `changed()`, which
  flushes every other connection, so the headless node passes edits between
  devices that are never online together.
- **`Peer::tidy` merges duplicates and tells connected devices.** `serve`
  calls it, then saves, shortly after every change (debounced 500 ms).
  Devices don't have to merge for themselves to get the result.
- **`serve` saves as changes arrive**, not only on Ctrl-C. The CLI test
  kills the server outright to check that.
- `Store::save` is atomic (write then rename). The device key is created
  once with mode 0600; an unreadable key is reported, never replaced.
- **Saving records and compacts.** `Store::save` records this device (its
  endpoint ID) in the graph's device list, then writes a shallow snapshot
  at the compaction point (devices silent for `ACTIVE`, 30 days, don't
  count). The graph in memory keeps its history, so open sessions aren't
  affected; only what's on disk is compacted. A blank replica records
  nothing, and `import` refuses one: it must join by syncing first.
- **A device behind its peer's history** gets `Error::Behind`. The side
  that finds its peer behind closes with code `BEHIND`, so the other can say
  why even if the close arrives before the peer's hello is read.
- **`--local ADDR`** binds to that address only, with no relays or address
  lookup. The tests use it on `127.0.0.1:0`, and iroh tests use
  `presets::Minimal` with relays disabled, so nothing leaves the machine.
- The node crate needs a newer Rust than the rest (see `rust-version`).
