#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
root="$PWD"
if [[ "${1:-}" != "" && "${1:-}" != "--build-only" ]]; then
  echo 'usage: run.sh [--build-only]' >&2; exit 1
fi
assets="${KINOGRAPH_WEB_ASSETS:-$root/target/browser-grid-site}"
manifest=experiments/browser-grid-prototype/Cargo.toml
export CARGO_TARGET_DIR="$root/target/browser-grid"
if ! rustup target list --installed | grep -q '^wasm32-unknown-unknown$'; then
  rustup target add wasm32-unknown-unknown
fi
tool="$root/target/browser-tools/bin/wasm-bindgen"
if [[ ! -x "$tool" ]]; then
  cargo install wasm-bindgen-cli --version 0.2.114 --locked --root "$root/target/browser-tools"
fi
cargo run --locked --release --manifest-path "$manifest" --bin bake -- "$assets"
RUSTFLAGS="--cfg=web_sys_unstable_apis" cargo build --locked --release --manifest-path "$manifest" --target wasm32-unknown-unknown --lib
"$tool" --target web --out-dir "$assets/pkg" target/browser-grid/wasm32-unknown-unknown/release/kinograph_browser_grid_prototype.wasm
[[ "${1:-}" == "--build-only" ]] && exit 0
exec bun experiments/browser-grid-prototype/serve.ts
