#!/bin/sh
# Builds the graph replica (crates/wasm) and its JavaScript bindings into
# lib/wasm, which is generated and not committed. Needs the wasm32 target and
# a wasm-bindgen CLI matching the crate's version.
set -eu
cd "$(dirname "$0")/../../.."
cargo build --locked -p quiddity-wasm --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir apps/ui/lib/wasm --out-name quiddity_wasm \
  target/wasm32-unknown-unknown/release/quiddity_wasm.wasm
