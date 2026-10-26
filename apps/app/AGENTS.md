# apps/app

The Tauri 2 shell. It holds the graph on disk and hands it to the UI; the UI
holds its own replica and answers its own queries (see `apps/ui` and
`crates/wasm`), so there is little here by design.

- **It is outside the Cargo workspace** (`exclude` in the root `Cargo.toml`),
  because it needs Tauri's system packages (webkit) that the core's CI
  shouldn't. It has its own `Cargo.lock`, its own CI job, and its own
  `cargo fmt`, `clippy` and `test` runs (from `src-tauri`).
- **Two commands, both raw bytes**: `load_graph` returns a snapshot;
  `push_updates` takes Loro updates as the request body, imports them and
  saves. Bytes cross IPC as raw bodies rather than JSON arrays. Updates that
  arrive out of order are held by Loro until their causes do.
- **Each push saves a whole snapshot**, atomically. That is fine at this size;
  an update log is the fix when it isn't. The UI pushes a few hundred
  milliseconds after typing stops, and on the window being hidden.
- `session.rs` knows nothing about Tauri, so it is tested on its own.
- **`QUIDDITY_GRAPH`** names a directory made by `quiddity-node` to work on
  instead of the one graph in the app's data directory.
- **Permissions are in `capabilities/default.json`.** The opener plugin lets the
  UI open `http(s)` and `mailto` links in the system's browser; without it
  following one would replace the UI with the page.
- **Don't run `cargo generate-lockfile`** here: it re-resolves everything.
  Let cargo add only what a new dependency needs.

## Trying it for real

A debug build loads the dev server unless the frontend is embedded:

```sh
pnpm ui build
cargo build --features tauri/custom-protocol      # in src-tauri
```

There is no display in a sandbox. `xvfb-run` with
`WEBKIT_DISABLE_COMPOSITING_MODE=1` runs the window, `xdotool` clicks and
types in it, and `import -window root` takes a screenshot. Check a saved edit
with `quiddity-node -g <graph> export <dir>` once the app has exited. The
`target` directory is large; `cargo mutants` copies the tree, so delete it
first.
