# Kinograph Agent Guide

Kinograph is an early Rust prototype for deterministic, code-first motion graphics. Its headless 1920x1080 benchmark scenes are rendered with `wgpu`, shaped with `cosmic-text`, temporally sampled for motion blur, and streamed to FFmpeg.

## Read First

- `CONTEXT.md` defines the domain language. Use its terms in code and documentation.
- `ARCHITECTURE.md` describes the current module boundaries and intentional non-abstractions.
- `PLAN.md` defines the product direction, milestones, scope cuts, and success criteria.
- `SCENE_PLANS.md` documents the lightweight Scene Program and persistent renderer workflow.
- `NOTES.md` records what the prototype has and has not proved.
- `PRIOR_ART.md` records the animation systems that should inform timeline and authoring API work.

Keep these documents accurate when a change alters a domain term, architectural boundary, validated finding, or milestone direction. Do not duplicate their detail here.

## Commands

```bash
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run --release
cargo run --release -- render effect-shows-errors
cargo run --release -- render chapter intro
cargo run --release -- render chapter basics
```

`cargo run --release` renders `output/kinograph-prototype.mp4` by default. Pass an output path as the first argument to override it. A full render requires:

- a working headless `wgpu` adapter
- `ffmpeg` with `libx264` on `PATH`
- CommitMono at the path currently declared in `crates/kinograph-render/src/scenes/mod.rs`; rendering falls back to the system monospace font if it is unavailable

Do not run the full render as routine validation when unit tests and static checks cover the change. Run it when changing rendering, typography, choreography, temporal sampling, readback, or encoding behavior.

## Architecture

- `crates/kinograph/src/code.rs`: stable line identity, code documents and snapshots, validation, and sampled line placement
- `crates/kinograph/src/composition.rs`: exact media time, immutable assets and clips, script/layer scheduling, cues, and cross-media composition
- `crates/kinograph/src/dsl.rs`: public Rust scene values, semantic targets, actor helpers, and lowering into scalar tracks
- `crates/kinograph/src/editor.rs`: typed editor recipe data lowering stable inline parts and logical ranges into Code Transitions
- `crates/kinograph/src/author.rs`: typed Scene Plan builder and stable actor/channel handles for lightweight Scene Programs
- `crates/kinograph/src/plan.rs`: versioned renderer-independent Scene Plan values and structured validation
- `crates/kinograph/src/state.rs`: deterministic arbitrary-time discrete State Tracks
- `crates/kinograph/src/terminal.rs`: lightweight terminal-recording recipe values for planned video media
- `crates/kinograph/src/timeline.rs`: relative Animation and explicit-time continuous Property Track compilation
- `crates/kinograph/src/motion.rs`: deterministic arbitrary-time analytic spring sampling with position and velocity
- `crates/kinograph/src/transcript.rs`: word timing ingestion and semantic cue lookup
- `crates/kinograph-render/src/render.rs`: concrete headless `wgpu` renderer, sprite compositor, and code annotations
- `crates/kinograph-render/src/render/effects/`: independent pixel recipes for interchangeable short annotation effects
- `crates/kinograph-render/src/render/task.rs`: concrete Effect Task recipe and compositing
- `crates/kinograph-render/src/render/terminal.rs`: concrete terminal recording and command-file presentation
- `crates/kinograph-render/src/render/ui.rs`: private bounds, inset, split, and flow layout primitives for pixel UI
- `crates/kinograph-render/src/render/ui/card.rs`: shared immediate-mode RGBA composition and projected card presentation used by editor and recorded-video producers
- `crates/kinograph-render/src/encode.rs`: concrete FFmpeg subprocess, raw RGBA protocol, and compiled audio placement
- `crates/kinograph-render/src/scenes/`: one concrete choreography Module per renderable scene plus shared sampling and encoding mechanics
- `crates/kinograph-render/src/scenes/effect_institute.rs`: private adapter from pinned published lesson artifacts into stable code, Task overlays, and stitched chapter schedules
- `crates/kinograph-render/src/main.rs`: command parsing, output selection, and scene dispatch
- `crates/kinograph/src/lib.rs`: lightweight public library boundary used by Rust Scene Programs
- `crates/kinograph-render/src/plan_runtime.rs`: Scene Plan inspection, validation, rendering, and persistent JSON server
- `crates/kinograph-render/src/plan_runtime/editor.rs`: concrete editor and attached pointer Scene Plan recipe
- `crates/kinograph-render/src/plan_runtime/terminal.rs`: concrete planned terminal-recording recipe and video source-time mapping
- `scenes/`: lightweight Rust Scene Programs that emit Scene Plans
- `scenes/hero/`: canonical editor-heavy Scene Program and generated plan used by the default render command
- `scenes/opencode-session-tool/`: narration-rich OpenCode v2 plugin lesson using planned terminal video, audio, text, and discrete state
- `crates/kinograph-render/src/scene.wgsl`: editor geometry and focus shader

Preserve these boundaries unless a concrete scene or second implementation demonstrates a better seam. The lightweight `kinograph` and heavyweight `kinograph-render` crates are a demonstrated process and compilation seam. Do not introduce a generic scene graph, renderer or encoder traits, plugins, or additional crate splits merely for future flexibility.

## Engineering Rules

- Prefer the smallest change that improves the benchmark scene or answers a stated prototype question.
- Keep concrete choreography visible in its scene Module. Move an operation into the DSL only after repeated scene usage demonstrates a deeper interface.
- Keep code identity and layout independent of glyphs, GPUs, colors, FFmpeg, and absolute screen coordinates.
- Keep layout responsible for target placement and motion responsible for trajectories.
- Preserve deterministic arbitrary-time sampling. Do not replace trajectories with stateful frame-by-frame integration.
- Carry both position and velocity when introducing interrupted or redirected motion.
- Add abstractions only after a real second use or implementation exposes the seam.
- Avoid unsafe code and codec bindings unless measured evidence shows the subprocess boundary is insufficient.
- Treat output media and build artifacts as generated files; keep them under ignored `output/` and `target/` directories.

## Effect Institute Ports

- Treat `/Users/kit/code/experiments/typescript/effect-institute` as the source of truth for published lesson choreography.
- Before porting or changing an animated-code lesson, read its `CLAUDE.md`, `docs/animation-patterns.md`, and `src/lib/animated-code/animation/stability.ts` maximum-stability rules.
- Run `bun narrate steps -c <chapter> <section>` in the Effect Institute repo and use the `Delta` output to identify exactly which slot content may change.
- Preserve maximum stability: text common to every state stays outside changing slots, stable lines retain one ID, and variable inline parts collapse or expand horizontally through width, opacity, and blur. Do not replace a whole line when only an inline range changes.
- Structural Code Transitions may add, remove, or reposition complete lines, but use Inline Reveals for slot-sized changes inside a stable line.
- Match the published motion constants: complete lines move only on `y` and transition opacity/blur with a 0.45-second zero-bounce spring; variable inline parts transition width/opacity/blur with a 0.4-second zero-bounce spring. Effect Institute lines do not enter or exit by sliding horizontally.

## Testing

- Add focused unit tests beside pure domain logic in `code.rs` and `motion.rs`.
- Test observable invariants such as stable identity, endpoint placement, velocity continuity, validation failures, and deterministic out-of-order sampling.
- For renderer or encoder changes, supplement automated checks with a targeted artifact render and inspect the result at full scale.
- When performance changes, compare the same resolution, frame count, temporal sample count, and build profile before claiming an improvement.

Before finishing a code change, run `cargo test --workspace`, `cargo fmt --check`, and strict workspace Clippy. State explicitly if GPU, font, or FFmpeg constraints prevented artifact-level verification.
