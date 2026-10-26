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
cargo test
PROPTEST_CASES=5000 cargo test --release   # deeper
cargo +nightly fuzz run block              # or outline, peer
```
