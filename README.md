# quiddity
Local first, multiplayer, second brain

See [docs/design.md](docs/design.md) for the current design.

## Layout

- `crates/syntax`: the block Markdown syntax (properties, links, block
  references). The reference parser.
- `crates/core`: the graph model on Loro, the index (names, link resolution,
  backlinks), property schemas, journals, duplicate merging, and text
  export/import.
- `crates/sync`: the sync protocol, independent of transport.
- `crates/node`: sync over iroh, and `quiddity-node`, a bare-bones command
  line and headless sync node.
- `apps/ui`: the Next.js UI (static export): three panes, with entrypoints,
  favourites and recents on the left, the current entity in the middle and
  open secondaries on the right. Read-only for now.
- `apps/app`: the Tauri 2 shell that hosts the UI.
- `fuzz`: fuzz targets for the parsers and the sync protocol.

## Trying it

```sh
cargo run -p quiddity-node -- -g ~/graph init
cargo run -p quiddity-node -- -g ~/graph import ~/logseq-graph
cargo run -p quiddity-node -- -g ~/graph serve --allow <device id>

# on another device
quiddity-node -g ~/graph init --blank    # prints this device's id
quiddity-node -g ~/graph sync <server id>
quiddity-node -g ~/graph export ~/export
```

## The app

```sh
pnpm install
pnpm ui dev          # the UI alone, in a browser, at http://localhost:3000
pnpm app dev         # the Tauri shell around it (needs Tauri's system packages)
```

The shell opens the one graph in the app's data directory, creating it if
need be. Set `QUIDDITY_GRAPH` to a directory made with `quiddity-node` to work
on that graph instead. Favourites are the entities with `favourite:: true`.
Run in a plain browser, the UI shows fixture data.

## Testing

Each layer is property-tested on its own, with system tests on top:

| Layer | Tests |
|---|---|
| syntax | generated values round-trip, alone and in prose (`crates/syntax/tests`); arbitrary text is fuzzed |
| outline format | generated outlines round-trip; arbitrary files are fuzzed to a fixed point |
| graph | random edits against an in-memory model |
| index | link resolution and backlinks against a brute-force oracle |
| sync | replicas converge however edits and syncs interleave; additions are never lost; journals merge |
| protocol | sessions over a simulated network with drops and relays; arbitrary peer input is fuzzed |
| system | graph → export → import round trip; real iroh connections; the CLI end to end |

```sh
cargo fmt --all --check
cargo clippy --all-targets
cargo nextest run                          # or cargo test
PROPTEST_CASES=5000 cargo nextest run --release   # deeper
cargo +nightly fuzz run block              # or outline, peer
PROPTEST_CASES=32 cargo mutants            # are the tests any good?
```

`cargo mutants` changes the code one small way at a time and checks that
some test notices. Each mutant is tested against its own crate's tests
only, so every layer has to stand up on its own. CI runs it over the lines
each pull request changes.
