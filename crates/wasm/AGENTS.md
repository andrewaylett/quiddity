# quiddity-wasm

`quiddity-view` for JavaScript: one class, `Replica`, which the UI holds as its
copy of the graph. It is glue and has no logic of its own. A feature belongs in
`quiddity-view` (where it is tested and mutation-tested natively); this crate
only gives it a JavaScript shape. That is why `.cargo/mutants.toml` excludes
it: its functions return JavaScript values and only run in a browser. It is
exercised end to end by the UI's browser tests.

- **The `wasm-bindgen` crate and CLI versions must match exactly.** The crate
  is pinned (`=0.2.x`); the CLI in CI and `mise.toml` is pinned to the same
  version. Change them together or the bindings fail to load.
- **Build with `pnpm ui wasm`** (`apps/ui/scripts/build-wasm.sh`): the crate
  for `wasm32-unknown-unknown`, then `wasm-bindgen --target web` into
  `apps/ui/lib/wasm`, which is generated and not committed.
- **Values are plain.** `js()` serialises to objects rather than `Map`s, and
  a missing value to `null` rather than `undefined`, to match the TypeScript
  types. Both settings matter: the default for each is the other one.
- **Errors become `JsError`** by `?`, with the message of the Rust error.
- **Offsets are UTF-16 units**, as in `quiddity-view`.
- `uuid` needs its `js` feature on `wasm32` for a source of randomness; it is
  enabled for that target only. `jiff` is built without its time zone
  database, which would be most of the binary and isn't used.
- Undo is enabled when a `Replica` is made. `clearUndo` is for edits that
  aren't the user's, such as creating today's journal at startup.
- To add a method: write it in `quiddity-view` with tests, wrap it here, then
  add it to the `Graph` wrapper in `apps/ui/lib/graph.ts`, which is where
  cache invalidation and notification happen.
