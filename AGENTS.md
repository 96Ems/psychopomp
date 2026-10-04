# Psychopomp Agent Guide

Psychopomp is a Rust prototype for deterministic, code-first motion graphics:
`wgpu` rendering, `cosmic-text` shaping, temporal sampling for motion blur, and
FFmpeg encoding.

## Read first

- `README.md`: the working example and documentation map.
- `CONTEXT.md`: domain terms. Use them in code and docs.
- `ARCHITECTURE.md`: the module map, boundaries, and intentional non-abstractions.
- `SCENE_PLANS.md`: authoring, presenting, rendering, and every recipe's channels.
- `EFFECTS.md` and `PRIOR_ART.md`: effect implementations and motion references.
- `docs/history/` and `perf/`: past experiments, as evidence rather than policy.
- `.agents/skills/psychopomp` and `.agents/skills/explainer-motion`: the reel
  workflow and motion rules. Load them before authoring a film.

Keep these accurate when a change alters a term, boundary, or finding.

## Commands

```sh
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run -p <scene-crate>                                   # write a plan or reel
cargo run --release -- plan validate <plan-or-reel.json>
cargo run --release -- plan render <plan-or-reel.json> output/<name>.mp4 --theme neutral
cargo run --release -- plan present <deck.json>              # native presentation
cargo run --release -- plan frame <plan-or-reel.json> <seconds> out.png --shutter
cargo run --release -- plan snapshot <plan-or-reel.json> <times> <dir> [--compare]
bun scripts/sheet.ts <plan-or-reel.json> <t1,t2,... | from:to:step> --shutter
bun scripts/narrate.ts <scene>/narration/script.json [--draft]
```

Rendering needs a working `wgpu` adapter and `ffmpeg` with `libx264`; fonts are
bundled. Do not render full films as routine validation; render when rendering,
typography, choreography, sampling, or encoding changes.

## Architecture

- `crates/psychopomp`: authoring values, Scene Plans, validation, timelines, and
  recipe data; no GPU.
- `crates/psychopomp-render` (binary `psychopomp`): `plan_runtime/` preflights and
  prepares plans, `render/` paints recipes, `exposure.rs` and `encode.rs` deliver video.
- `scenes/`: one Rust Scene Program per film or showroom.
- A new recipe adds values and an authoring handle in `crates/psychopomp`, strict
  preflight in `plan_runtime/<x>.rs`, pixels in `render/<x>.rs`, and a showroom in
  `scenes/<x>/`.

Do not add a generic scene graph, renderer traits, plugins, or crate splits for
future flexibility. Add an abstraction when a second real use exposes the seam.

## Rules

- Make the smallest change that improves a real scene or answers a stated question.
- Keep choreography visible in its Scene Program; promote an operation into
  `psychopomp` only after repeated use.
- Sampling is a pure function of time. Never integrate state frame by frame.
  Interrupted motion carries both position and velocity.
- Reusable interpolation, easing, and geometry belong in `psychopomp::math`; no
  private lerps or easings in renderers or scenes.
- Code identity and layout stay independent of glyphs, GPUs, colors, and screen
  coordinates.
- Generated plans and media go in ignored `target/` and `output/`.
- Never commit secrets. Narration keys come from the environment.

## Stable code between steps

Viewers must be able to follow the same code through a change.

- Keep stable line and part IDs. Reveal only what changes: for
  `Effect<number>` → `Effect<number, Error>`, reveal `, Error` and keep the rest.
- A retained part may move to make room, but never disappears and reappears.
- Retarget interrupted motion from its current position and velocity; settled
  channels stay settled unless retargeted.
- Verify forward, backward, skipped, and rapid `A → B → A → C` navigation down to
  pixels. Run `psychopomp plan steps <plan.json>` for code presentations.

## Verify

- Add focused unit tests beside pure logic for invariants: identity, endpoints,
  velocity continuity, validation failures, and out-of-order sampling.
- For renderer changes, inspect a targeted artifact at full scale. For
  behavior-preserving refactors, `plan snapshot` before and `--compare` after:
  rendering is deterministic, so any changed pixel is real.
- Compare performance only at identical resolution, frames, samples, and profile.

Before finishing, run the three checks above and say if GPU, font, or FFmpeg
limits prevented artifact-level verification.
