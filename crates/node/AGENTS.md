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
- **`--local ADDR`** binds to that address only, with no relays or address
  lookup. The tests use it on `127.0.0.1:0`, and iroh tests use
  `presets::Minimal` with relays disabled, so nothing leaves the machine.
- The node crate needs a newer Rust than the rest (see `rust-version`).
