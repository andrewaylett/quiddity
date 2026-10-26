#!/bin/sh
# Checks what building and running Quiddity needs that mise can't install, and
# says how to get it. Exits non-zero if anything is missing.
set -u
cd "$(dirname "$0")/.."

missing=0
problem() {
  printf 'missing: %s\n' "$1" >&2
  [ -n "${2:-}" ] && printf '  %s\n' "$2" >&2
  missing=1
}

for tool in cargo node pnpm; do
  command -v "$tool" >/dev/null 2>&1 || problem "$tool" "run \`mise install\`"
done

# The UI embeds the core compiled to WASM.
if command -v rustup >/dev/null 2>&1; then
  rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown ||
    problem "the wasm32-unknown-unknown Rust target" "rustup target add wasm32-unknown-unknown (or \`mise install\`)"
fi

# The CLI must be the version the wasm-bindgen crate is pinned to.
want=$(sed -n 's/^wasm-bindgen = "=\([0-9.]*\)".*/\1/p' crates/wasm/Cargo.toml)
have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')
if [ -z "$have" ]; then
  problem "wasm-bindgen $want" "\`mise install\` (compiles it, a few minutes the first time)"
elif [ "$have" != "$want" ]; then
  problem "wasm-bindgen $want (found $have)" "\`mise install\`, or cargo install wasm-bindgen-cli --version $want --locked"
fi

# Tauri's system libraries, on Linux. Windows and macOS have what it needs.
if [ "$(uname -s)" = Linux ]; then
  if ! command -v pkg-config >/dev/null 2>&1; then
    problem "pkg-config" "sudo apt-get install pkg-config"
  elif ! pkg-config --exists webkit2gtk-4.1 gtk+-3.0; then
    problem "Tauri's system libraries (webkit2gtk 4.1 and GTK 3)" \
      "Debian or Ubuntu: sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev
  Others: https://tauri.app/start/prerequisites/#linux"
  fi
fi

if [ "$missing" -eq 0 ]; then
  echo "Everything needed is here."
fi
exit "$missing"
