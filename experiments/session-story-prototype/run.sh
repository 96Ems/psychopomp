#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
if [[ "${1:-}" != "--preview-only" ]]; then
  CARGO_TARGET_DIR="$root/target/session-story-build" cargo run --release \
    --manifest-path experiments/session-story-prototype/Cargo.toml \
    -- "$root/output/session-story-prototype" "${@}"
fi
if [[ "${1:-}" == "--stills" ]]; then exit 0; fi
exec bun experiments/session-story-prototype/preview.ts
