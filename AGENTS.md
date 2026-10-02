# Kinograph Agent Guide

Kinograph is an early Rust prototype for deterministic, code-first motion graphics. Its headless 1920x1080 benchmark scenes are rendered with `wgpu`, shaped with `cosmic-text`, temporally sampled for motion blur, and streamed to FFmpeg.

## Read First

Start with `README.md` for the working example and documentation map. Use the
domain and current architecture as contracts; consult relevant experiment
history for evidence rather than treating earlier trials as current policy.

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
cargo run -p kinograph-effect-succeed-slides -- target/effect-succeed-slides.json
cargo run --release -- plan present target/effect-succeed-slides.json
cargo run -- plan steps target/effect-succeed-slides.json
cargo run -p kinograph-interactive-showcase
cargo run --release -- plan present target/interactive-showcase/deck.json
cargo run -p kinograph-data-modeling
cargo run --release -- plan present target/data-modeling/deck.json
bash scenes/component-prototypes/run.sh
cargo run -p kinograph-keyed-grid -- --styles
cargo run --release -- plan present target/grid-styles/deck.json
cargo run -p kinograph-component-prototypes -- --slideshow
cargo run --release -- plan present target/slideshow-components/deck.json
cargo run -p kinograph-pr-walkthrough
cargo run -p kinograph-rolling-number
cargo run --release -- plan render target/rolling-number.json output/rolling-number.mp4 --theme opencode
cargo run --release -- plan render scenes/pr-walkthrough/pr-walkthrough.reel.json output/pr-walkthrough.mp4 --theme opencode
bun scripts/narrate.ts <scene>/narration/script.json [--draft]
bun scripts/sheet.ts <plan-or-reel.json> <from:to:step | t1,t2,...> [--theme NAME] [--crop x,y,w,h] [--shutter]
cargo run --release -- plan frame <plan-or-reel.json> <seconds> out.png --shutter
cargo run --release -- plan snapshot <plan-or-reel.json> <times> <dir> [--compare] [--shutter]
```

`cargo run --release` renders `output/kinograph-prototype.mp4` by default. Pass an output path as the first argument to override it. A full render requires:

- a working headless `wgpu` adapter
- `ffmpeg` with `libx264` on `PATH`
- nothing for fonts: CommitMono (400/700, roman and italic) is bundled in `assets/fonts` and compiled in by `render/fonts.rs`; installed fonts only fill glyphs it lacks

Do not run the full render as routine validation when unit tests and static checks cover the change. Run it when changing rendering, typography, choreography, temporal sampling, readback, or encoding behavior.

## Architecture

- `crates/kinograph/src/code.rs`: stable line identity, code documents and snapshots, validation, and sampled line placement
- `crates/kinograph/src/composition.rs`: exact media time, immutable assets and clips, script/layer scheduling, cues, and cross-media composition
- `crates/kinograph/src/dsl.rs`: public Rust scene values, semantic targets, actor helpers, and lowering into scalar tracks
- `crates/kinograph/src/editor.rs`: typed editor recipe data lowering stable inline parts and logical ranges into Code Transitions
- `crates/kinograph/src/editor/compiled.rs`: shared validated catalog, reveal ranges, and legacy/keyed placement used by inspection and rendering
- `crates/kinograph/src/editor/stability.rs`: GPU-free step deltas and heuristic common-text stability warnings
- `crates/kinograph/src/task.rs`: typed planned Task state schedules
- `crates/kinograph/src/grid.rs`: finite keyed product catalogs and semantic Grid Snapshots
- `crates/kinograph/src/value.rs`: immutable Value Token recipe data for finite teaching diagrams
- `crates/kinograph/src/author.rs`: typed Scene Plan builder and stable actor/channel handles for lightweight Scene Programs
- `crates/kinograph/src/plan.rs`: versioned renderer-independent Scene Plan values and structured validation
- `crates/kinograph/src/plan/channels.rs`: exact scalar-event lowering and opt-in snapshot-destination reduction; raw event ordering remains distinct
- `crates/kinograph/src/state.rs`: deterministic arbitrary-time discrete State Tracks
- `crates/kinograph/src/playback.rs`: interruptible step destinations, continuous track retargeting, and a pausable local presentation clock
- `crates/kinograph/src/terminal.rs`: lightweight terminal-recording recipe values for planned video media
- `crates/kinograph/src/deployment.rs`: typed deployment-queue recipe values, semantic snapshots, and authoring handle
- `crates/kinograph/src/timeline.rs`: relative Animation and explicit-time continuous Property Track compilation
- `crates/kinograph/src/timeline/retarget.rs`: shared cancellation-safe numeric schedule for Playback and authored resting entrances
- `crates/kinograph/src/motion.rs`: deterministic arbitrary-time analytic spring sampling with position and velocity
- `crates/kinograph/src/transcript.rs`: word timing ingestion, word and phrase cue lookup
- `crates/kinograph/src/sequence.rs`: Sequence Diagram recipe values, slot geometry, validation, and the `SequenceActor` authoring handle
- `crates/kinograph/src/caption.rs`: Caption recipe values and the `CaptionActor` authoring handle (typing, show, hide)
- `crates/kinograph/src/rolling.rs`: Rolling Number recipe values, value tokenization, the closed-form roll compiler, and the `RollingNumberActor` handle (`roll`, show, hide)
- `crates/kinograph/src/math.rs` and `math/`: shared motion and geometry math (glam vectors, lerp/remap/smoothstep, easing, closed-form dynamics such as the settling spring, arc-length curves, shape ports and connectors, deterministic hash)
- `crates/kinograph/src/stage.rs`: Stage elements, strict channels, perspective camera, orb geometry, the packet clock (`stage::packet`), and the `StageActor` authoring handle (`settle_in`, `ease`, `clock`, `connect`, `send`, `hit`, `twang`, `land`)
- `crates/kinograph/src/tone.rs`: semantic Tone roles shared by explainer recipes
- `crates/kinograph/src/highlight.rs`: line-local TypeScript highlighting into editor spans
- `crates/kinograph-render/src/plan_runtime/reel.rs`: Reel preparation, layer mixing, media retiming, and reel frame/video delivery
- `crates/kinograph-render/src/plan_runtime/sequence.rs` and `caption.rs`: strict-channel preflight for the explainer overlays
- `crates/kinograph-render/src/render/sequence.rs` and `render/caption.rs`: Sequence Diagram and Caption pixels
- `crates/kinograph-render/src/plan_runtime/rolling.rs` and `render/rolling.rs`: Rolling Number preflight/compilation and its masked, smeared wheels
- `crates/kinograph-render/src/plan_runtime/stage.rs`: Stage root preflight and preparation
- `crates/kinograph-render/src/render/stage.rs`, `stage.wgsl`, `stage_post.wgsl`: Stage primitives, HDR bloom, and composite; `KINOGRAPH_SHADER_DIR` loads the WGSL live
- `crates/kinograph/src/effects/`: GPU-free special-effect clocks and particle poses; shared dynamics stay in `kinograph::math::dynamics`
- `crates/kinograph-render/src/render/effects/*.wgsl`: binding-free noise, combustion, and pressure Modules, composed by the Stage shaders; see `EFFECTS.md`
- `crates/kinograph-render/src/render.rs`: concrete headless `wgpu` renderer, sprite compositor, and code annotations
- `crates/kinograph-render/src/render/effects/`: independent pixel recipes for interchangeable short annotation effects
- `crates/kinograph-render/src/render/task.rs`: concrete Effect Task recipe and compositing
- `crates/kinograph-render/src/render/grid.rs`: opaque connected 3D grid, sampled-bounds centering, and cached symbols/labels
- `crates/kinograph-render/src/render/grid/edges.rs`: centered screen-space grid strokes, nearest-depth selection, and shared-edge coverage union
- `crates/kinograph-render/src/render/value.rs`: Value Token tiles using shared card coverage and cached fractional text
- `crates/kinograph-render/src/render/theme.rs`: named native/export paint palettes; no layout or motion
- `crates/kinograph-render/src/render/rich_text.rs`: bounded Markdown shaping, decoration, and theme-aware glyph cache
- `crates/kinograph-render/src/render/text.rs` and `text/raster.rs`: typed plain-text cache and exact native glyph rasterization shared with the experimental bake
- `crates/kinograph-render/src/render/venn.rs`: sampled rounded-set geometry and exact intersection hatching
- `crates/kinograph-render/src/render/header.rs`: fixed-edge line/word rises and mirrored, fading reflection ink
- `crates/kinograph-render/src/render/terminal.rs`: concrete terminal recording and command-file presentation
- `crates/kinograph-render/src/render/deployment_queue.rs`: concrete state-driven deployment dashboard UI Surface
- `crates/kinograph-render/src/render/ui.rs`: private bounds, inset, split, and terminal line-flow primitives for pixel UI
- `crates/kinograph-render/src/render/ui/card.rs`: shared immediate-mode RGBA composition and projected card presentation used by editor, recorded-video, and simulated-UI producers
- `crates/kinograph-render/src/encode.rs`: concrete FFmpeg subprocess, raw RGBA protocol, and compiled audio placement
- `crates/kinograph-render/src/scenes/`: one concrete choreography Module per renderable scene plus shared sampling and encoding mechanics
- `crates/kinograph-render/src/scenes/effect_institute.rs`: private adapter from pinned published lesson artifacts into stable code, Task overlays, and stitched chapter schedules
- `crates/kinograph-render/src/main.rs`: command parsing, output selection, and scene dispatch
- `crates/kinograph/src/lib.rs`: lightweight public library boundary used by Rust Scene Programs
- `crates/kinograph-render/src/plan_runtime.rs`: Scene Plan inspection, validation, rendering, and persistent JSON server
- `crates/kinograph-render/src/plan_runtime/preflight.rs`: owned typed recipe inputs, root selection, references, and native eligibility before resources
- `crates/kinograph-render/src/plan_runtime/generated.rs`: generated-channel reservation/insertion, including the explicit Task position override
- `crates/kinograph-render/src/plan_runtime/delivery.rs`: PNG and MP4 delivery from a prepared scene
- `crates/kinograph-render/src/plan_runtime/presentation.rs`: native winit window, step navigation, and smooth/pixelated display filtering
- `crates/kinograph-render/src/plan_runtime/presentation/worker.rs`: persistent render worker with bounded in-flight sampling and immutable timeline revisions
- `crates/kinograph-render/src/plan_runtime/presentation/scheduler.rs`: GPU-free request eligibility, invalidation, completion freshness, and deadlines
- `crates/kinograph-render/src/plan_runtime/presentation/debug.rs`: sample-coherent optional native motion diagnostics
- `crates/kinograph-render/src/plan_runtime/presentation/gpu.rs`: native wgpu surface and GPU-backed smooth/pixelated frame presentation
- `crates/kinograph-render/src/plan_runtime/editor.rs`: concrete editor and attached pointer Scene Plan recipe
- `crates/kinograph-render/src/plan_runtime/attachments.rs`: private companion-track compilation for layout-aware semantic coordinates
- `crates/kinograph-render/src/plan_runtime/task.rs`: Task state schedules lowered into interruptible scalar visual destinations
- `crates/kinograph-render/src/plan_runtime/grid.rs`: GPU-free keyed grid layout and continuous destination compilation
- `crates/kinograph-render/src/plan_runtime/grid/table.rs`: fixed-anchor table placement, display headings, and padding/alignment over the same grid catalog
- `crates/kinograph-render/src/plan_runtime/value.rs`: Value Token validation and ordinary scalar-channel sampling
- `crates/kinograph-render/src/plan_runtime/header.rs`: header word tracks and opt-in resting-entrance delays with cancellation
- `crates/kinograph-render/src/plan_runtime/diagram.rs`: finite box/wire preflight and recipe-owned native start delays
- `crates/kinograph-render/src/render/diagram.rs` and `diagram.wgsl`: shared native/browser GPU boxes, sampled ports, wire traces and bare flat/isometric views
- `crates/kinograph-render/src/plan_runtime/terminal.rs`: concrete planned terminal-recording recipe and video source-time mapping
- `crates/kinograph-render/src/plan_runtime/deployment_queue.rs`: deployment snapshot validation, private track compilation, and rendering adapter
- `crates/kinograph-render/src/plan_runtime/keyed_layout.rs`: private stable keyed position and presence track compiler
- `scenes/`: lightweight Rust Scene Programs that emit Scene Plans
- `scenes/hero/`: canonical editor-heavy Scene Program and generated plan used by the default render command
- `scenes/effect-succeed-slides/`: Effect Institute code-reveal adaptation proving manual presentation and video export from one source
- `scenes/interactive-showcase/`: four-slide native deck covering inline reveals, Task lifecycle/retry, parallel Tasks, and keyed code edits
- `scenes/keyed-grid/`: native row/table/3D-layer growth and product-reassociation proof
- `scenes/data-modeling/`: seven-slide types/cardinality, finite correspondence, joystick, sum/product, and illegal-state adaptation
- `scenes/component-prototypes/`: provisional reusable Typeset, Collection, and Connector showroom; payloads and adapters remain in the three `component_prototype.rs` modules until visual approval
- `scenes/opencode-architecture/`: four-step Daemon / merge port using the provisional box-and-wire diagram surface
- `scenes/opencode-session-tool/`: rapid-fire OpenCode v2 hot-reload proof using split Vim/OpenCode terminal video, layered SFX, text, and discrete state
- `scenes/deployment-queue/`: canonical state-driven simulated UI proof with keyed insertion, phase replacement, failure focus, and retry
- `scenes/rolling-number/`: Rolling Number showroom: roll up and down, a mid-roll redirect, a carry into a new place, and a shrink
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
- Put reusable interpolation, easing, and geometry in `kinograph::math` and compose it; do not add private lerps, easings, or connector math to renderers or Scene Programs. Keep renderers as small per-element helpers, as in `render/stage.rs`.
- Avoid unsafe code and codec bindings unless measured evidence shows the subprocess boundary is insufficient.
- Treat output media and build artifacts as generated files; keep them under ignored `output/` and `target/` directories.

## Maximum Stability Between Steps

This is the default for every code presentation and video, not only Effect Institute ports. The viewer should be able to keep following the same code through a change. Preserve identity first; add motion only where the change requires it.

- Inspect all step variants before partitioning a line. Keep common prefixes, infixes, suffixes, delimiters, URLs, and indentation outside changing inline parts. For `Effect<number>` → `Effect<number, Error>`, reveal only `, Error`; retain `Effect<number` and `>`.
- Keep stable `LineId` and `PartId` values. Identical text in two semantic roles is not automatically the same part; do not match repeated punctuation or repeated `never` tokens arbitrarily. Kinograph currently uses authored identity, not automatic text diffing.
- Separate the content delta from necessary layout movement. A retained suffix may move to make room, but must not disappear and reappear. Do not replace, fade, or blur a complete line for a slot-sized change, or recenter unaffected code without an explicit choreography reason.
- Retarget interrupted motion from current position and velocity. Unchanged destinations keep their trajectories. Previous is an animated destination change, not a seek; only explicit Replay resets to an entry pose.
- Once a trajectory reports settled at a time, later samples must stay settled unless it is retargeted. Starting another channel must not wake an unchanged one. Establish settling deterministically, not with a frame-history-dependent latch.
- Review continuity all the way to pixels. Fractional positions, reveal widths, and blur must not become visible stair-steps through premature rounding. Smooth final-window scaling cannot recover motion precision already lost in the compositor; deliberately pixelated display filtering is a separate choice.
- Check attached pointers and highlights against the currently sampled visible layout, including collapsed parts and moving lines. Measuring the fully expanded backing text alone does not establish correct attachment.
- Verify forward, backward, skipped-step, and rapid `A → B → A → C` navigation, including interruptions before settling. Check retained identity, minimal changed ranges, position/velocity continuity, and pixels just before/at/after boundaries. Endpoint images, scalar-only tests, and a high FPS counter are not sufficient proof.
- Run `kinograph plan steps <plan.json>` for code presentations. Inspect `beforeDelta`, `delta`, changed part IDs, line positions, and unsettled-hold warnings. Common-text warnings require semantic judgment, not automatic identity merging. Use timed `EditorSnapshotPlan` values for multi-step line-order changes.

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

## Effect Institute Ports

- Treat `/Users/kit/code/experiments/typescript/effect-institute` as the source of truth for published lesson choreography.
- Before porting or changing an animated-code lesson, read its `CLAUDE.md`, `.claude/skills/content-animation/SKILL.md`, and the **Stability Principle** in that skill's `references/animation-dsl.md`. Also read `docs/animation-patterns.md` and `src/lib/animated-code/animation/stability.ts`; the latter is a heuristic warning checker, not a guarantee of maximum stability.
- Run `bun narrate steps -c <chapter> <section>` in the Effect Institute repo and use the `Delta` output to identify exactly which slot content may change.
- Require the smallest possible changed-slot markers in the `Delta` output. If the CLI is unavailable, report that limitation and inspect the source variants directly; do not silently claim stability validation or repair unrelated dependencies.
- Structural Code Transitions may add, remove, or reposition complete lines, but use Inline Reveals for slot-sized changes inside a stable line.
- Match the published motion constants: complete lines move only on `y` and transition opacity/blur with a 0.45-second zero-bounce spring; variable inline parts transition width/opacity/blur with a 0.4-second zero-bounce spring. Effect Institute lines do not enter or exit by sliding horizontally.

## Testing

- Add focused unit tests beside pure domain logic in `code.rs` and `motion.rs`.
- Test observable invariants such as stable identity, endpoint placement, velocity continuity, validation failures, and deterministic out-of-order sampling.
- For renderer or encoder changes, supplement automated checks with a targeted artifact render and inspect the result at full scale.
- When performance changes, compare the same resolution, frame count, temporal sample count, and build profile before claiming an improvement.
- For behavior-preserving renderer refactors, write `plan snapshot` frames before the change and run `--compare` after; rendering is deterministic, so any changed pixel is a real change. Use `--shutter` stills to judge motion blur instead of encoding a whole film.

Before finishing a code change, run `cargo test --workspace`, `cargo fmt --check`, and strict workspace Clippy. State explicitly if GPU, font, or FFmpeg constraints prevented artifact-level verification.
