# quiddity
Local first, multiplayer, second brain

See [docs/design.md](docs/design.md) for the current design.

## Layout

- `crates/syntax`: the block Markdown syntax (properties, links, block
  references). The reference parser.
- `crates/core`: the graph model on Loro, the index (names, link resolution,
  backlinks), property schemas, journals, duplicate merging, and text
  export/import.

## Development

```sh
cargo fmt --all --check
cargo clippy --all-targets
cargo test
```
