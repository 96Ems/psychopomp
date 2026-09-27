#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo run -p kinograph-component-prototypes
cargo run --release -- plan present target/component-prototypes/deck.json "$@"
