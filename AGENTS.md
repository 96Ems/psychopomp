# Psychopomp Agent Guide

Psychopomp is an early Rust prototype for deterministic, code-first motion graphics. Its headless 1920x1080 benchmark scenes are rendered with `wgpu`, shaped with `cosmic-text`, temporally sampled for motion blur, and streamed to FFmpeg.

## Read First

Start with `README.md` for the working example and documentation map. Use the
domain and current architecture as contracts; consult relevant experiment
history for evidence rather than treating earlier trials as current policy.

- `CONTEXT.md` defines the domain language. Use its terms in code and documentation.
- `ARCHITECTURE.md` is the module map and describes the current boundaries and intentional non-abstractions.
- `SCENE_PLANS.md` documents the lightweight Scene Program and persistent renderer workflow.
- `PRIOR_ART.md` records the animation systems that should inform timeline and authoring API work.
- `docs/history/` keeps the original plan (`PLAN.md`) and experiment findings (`NOTES.md`) as evidence, not current policy.
- `scenes/effect-succeed-slides` shows how an Effect Institute lesson is ported as a Scene Plan; `/Users/kit/code/experiments/typescript/effect-institute` remains the source of truth for its choreography.

Keep these documents accurate when a change alters a domain term, architectural boundary, or validated finding. Do not duplicate their detail here.

## Commands

```bash
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run --release
cargo run -p psychopomp-effect-succeed-slides -- target/effect-succeed-slides.json
cargo run --release -- plan present target/effect-succeed-slides.json
cargo run -- plan steps target/effect-succeed-slides.json
cargo run -p psychopomp-interactive-showcase
cargo run --release -- plan present target/interactive-showcase/deck.json
cargo run -p psychopomp-data-modeling
cargo run --release -- plan present target/data-modeling/deck.json
bash scenes/component-prototypes/run.sh
cargo run -p psychopomp-keyed-grid -- --styles
cargo run --release -- plan present target/grid-styles/deck.json
cargo run -p psychopomp-component-prototypes -- --slideshow
cargo run --release -- plan present target/slideshow-components/deck.json
cargo run -p psychopomp-pr-walkthrough
cargo run -p psychopomp-rolling-number
cargo run --release -- plan render target/rolling-number.json output/rolling-number.mp4 --theme opencode
cargo run --release -- plan render scenes/pr-walkthrough/pr-walkthrough.reel.json output/pr-walkthrough.mp4 --theme opencode
bun scripts/narrate.ts <scene>/narration/script.json [--draft]
bun scripts/sheet.ts <plan-or-reel.json> <from:to:step | t1,t2,...> [--theme NAME] [--crop x,y,w,h] [--shutter]
cargo run --release -- plan frame <plan-or-reel.json> <seconds> out.png --shutter
cargo run --release -- plan snapshot <plan-or-reel.json> <times> <dir> [--compare] [--shutter]
```

`cargo run --release` renders `output/psychopomp-prototype.mp4` by default. Pass an output path as the first argument to override it. A full render requires:

- a working headless `wgpu` adapter
- `ffmpeg` with `libx264` on `PATH`
- nothing for fonts: CommitMono (400/700, roman and italic) is bundled in `assets/fonts` and compiled in by `render/fonts.rs`; installed fonts only fill glyphs it lacks

Do not run the full render as routine validation when unit tests and static checks cover the change. Run it when changing rendering, typography, choreography, temporal sampling, readback, or encoding behavior.

## Architecture

`ARCHITECTURE.md` is the single module map; update it when a Module's responsibility changes.

- `crates/psychopomp`: lightweight authoring values, Scene Plans, validation, timelines, playback, and recipe data; no GPU.
- `crates/psychopomp-render` (binary `psychopomp`): `plan_runtime/` preflights and prepares plans, `render/` paints recipes, `exposure.rs` and `encode.rs` deliver video.
- `scenes/`: one Rust Scene Program per scene or showroom, each emitting a Scene Plan or Reel.
- A new recipe follows the existing pattern: values and an authoring handle in `crates/psychopomp`, strict preflight in `plan_runtime/<x>.rs`, pixels in `render/<x>.rs`, and a showroom under `scenes/<x>/`.

Preserve these boundaries unless a concrete scene or second implementation demonstrates a better seam. The lightweight `psychopomp` and heavyweight `psychopomp-render` crates are a demonstrated process and compilation seam. Do not introduce a generic scene graph, renderer or encoder traits, plugins, or additional crate splits merely for future flexibility.

## Engineering Rules

- Prefer the smallest change that improves the benchmark scene or answers a stated prototype question.
- Keep concrete choreography visible in its Scene Program. Move an operation into `psychopomp` only after repeated scene usage demonstrates a deeper interface.
- Keep code identity and layout independent of glyphs, GPUs, colors, FFmpeg, and absolute screen coordinates.
- Keep layout responsible for target placement and motion responsible for trajectories.
- Preserve deterministic arbitrary-time sampling. Do not replace trajectories with stateful frame-by-frame integration.
- Carry both position and velocity when introducing interrupted or redirected motion.
- Add abstractions only after a real second use or implementation exposes the seam.
- Put reusable interpolation, easing, and geometry in `psychopomp::math` and compose it; do not add private lerps, easings, or connector math to renderers or Scene Programs. Keep renderers as small per-element helpers, as in `render/stage.rs`.
- Avoid unsafe code and codec bindings unless measured evidence shows the subprocess boundary is insufficient.
- Treat output media and build artifacts as generated files; keep them under ignored `output/` and `target/` directories.

## Maximum Stability Between Steps

This is the default for every code presentation and video, not only Effect Institute ports. The viewer should be able to keep following the same code through a change. Preserve identity first; add motion only where the change requires it.

- Inspect all step variants before partitioning a line. Keep common prefixes, infixes, suffixes, delimiters, URLs, and indentation outside changing inline parts. For `Effect<number>` → `Effect<number, Error>`, reveal only `, Error`; retain `Effect<number` and `>`.
- Keep stable `LineId` and `PartId` values. Identical text in two semantic roles is not automatically the same part; do not match repeated punctuation or repeated `never` tokens arbitrarily. Psychopomp currently uses authored identity, not automatic text diffing.
- Separate the content delta from necessary layout movement. A retained suffix may move to make room, but must not disappear and reappear. Do not replace, fade, or blur a complete line for a slot-sized change, or recenter unaffected code without an explicit choreography reason.
- Retarget interrupted motion from current position and velocity. Unchanged destinations keep their trajectories. Previous is an animated destination change, not a seek; only explicit Replay resets to an entry pose.
- Once a trajectory reports settled at a time, later samples must stay settled unless it is retargeted. Starting another channel must not wake an unchanged one. Establish settling deterministically, not with a frame-history-dependent latch.
- Review continuity all the way to pixels. Fractional positions, reveal widths, and blur must not become visible stair-steps through premature rounding. Smooth final-window scaling cannot recover motion precision already lost in the compositor; deliberately pixelated display filtering is a separate choice.
- Check attached pointers and highlights against the currently sampled visible layout, including collapsed parts and moving lines. Measuring the fully expanded backing text alone does not establish correct attachment.
- Verify forward, backward, skipped-step, and rapid `A → B → A → C` navigation, including interruptions before settling. Check retained identity, minimal changed ranges, position/velocity continuity, and pixels just before/at/after boundaries. Endpoint images, scalar-only tests, and a high FPS counter are not sufficient proof.
- Run `psychopomp plan steps <plan.json>` for code presentations. Inspect `beforeDelta`, `delta`, changed part IDs, line positions, and unsettled-hold warnings. Common-text warnings require semantic judgment, not automatic identity merging. Use timed `EditorSnapshotPlan` values for multi-step line-order changes.

## Expressive Content Motion

- Normal paragraphs should appear sharply and quickly, not inherit the word/slot
  blur treatment. The native showroom uses 160 ms zero-bounce prose fades; the
  approved masked-rise header retains its separate 400 ms motion and fade blur.

- Match a known reference's operation-specific timings and overlap before inventing new staging. Do not force every transition into an outgoing-content → container → incoming-content sequence; unnecessary waits can destroy its snap.
- Content needs its own scale, opacity, and blur pose, independent of container geometry. Use rotation selectively for symbols; do not automatically spin result text, labels, or stable code.
- Keep unchanged content still. Resolve competing payloads rather than layering two readable silhouettes in the same space.
- Rolling text should pass through a stationary clipped window with linear edge fades (`text.data.verticalMask`), not carry a fade with the moving line or paint a dark overlay over unrelated actors. Preserve the existing motion timing when adding this spatial treatment.
- Give scale, opacity, blur, and secondary geometry independent tracks when the reference uses different springs. A second easing or progress window on an already animated value changes its timing; it is not a neutral implementation detail.
- Richer staging must remain interruptible and deterministic. Avoid delayed callbacks and direction-dependent pose resets; test each visible channel and its velocity through reversal.
- Inspect normal-speed and slowed/stepped artifacts, not only endpoint images. The motion-graphics sources and their application are recorded in `PRIOR_ART.md`.

## Testing

- Add focused unit tests beside pure domain logic in `code.rs` and `motion.rs`.
- Test observable invariants such as stable identity, endpoint placement, velocity continuity, validation failures, and deterministic out-of-order sampling.
- For renderer or encoder changes, supplement automated checks with a targeted artifact render and inspect the result at full scale.
- When performance changes, compare the same resolution, frame count, temporal sample count, and build profile before claiming an improvement.
- For behavior-preserving renderer refactors, write `plan snapshot` frames before the change and run `--compare` after; rendering is deterministic, so any changed pixel is a real change. Use `--shutter` stills to judge motion blur instead of encoding a whole film.

Before finishing a code change, run `cargo test --workspace`, `cargo fmt --check`, and strict workspace Clippy. State explicitly if GPU, font, or FFmpeg constraints prevented artifact-level verification.
