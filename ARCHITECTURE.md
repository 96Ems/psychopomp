# Prototype Architecture

Kinograph is organized around a small set of Modules whose Interfaces correspond to demonstrated change seams: code identity, media composition, motion trajectories, scalar tracks, rendering, and encoding.

Video export and native interactive presentation share Scene Plan preparation, scalar tracks, and visual recipes. Winit owns the native window and input; a wgpu surface displays Rust-rendered RGBA pixels. There is no browser playback layer or second animation implementation.

## Scene Programs And Rendering Compile Separately

The workspace has one demonstrated package seam. `crates/kinograph` is a lightweight library containing authoring values, versioned Scene Plans, validation, exact composition time, continuous Property Tracks, discrete State Tracks, and stable code identity. `crates/kinograph-render` contains `wgpu`, `cosmic-text`, video decoding, FFmpeg encoding, built-in renderer recipes, and the CLI.

```text
Rust Scene Program -> Scene Plan -> persistent kinograph-render process
```

A Scene Program remains ordinary Rust and may perform arbitrary calculations before emitting a plan. The Scene Plan is compiled output rather than a replacement authoring language. Its renderer-independent Interface contains stable actors, continuous channels, state channels, exact cues, and media placements. Renderer Recipe payloads remain opaque to the core and are interpreted only by concrete adapters in `kinograph-render`.

`crates/kinograph-render/src/plan_runtime.rs` validates and inspects plans without initializing a GPU, renders exact global Render Windows, and provides a newline-delimited JSON server that retains the GPU device and font caches across requests. Intersecting media is trimmed and rebased to output zero while visual sampling remains on the original global clock. Explicit event selection uses a `f64` seconds clock derived from serialized integer nanoseconds; scalar spring positions and velocities remain `f32`.

Planned audio and visual media share the same exact source and timeline ranges but have different concrete consumers. Audio lowers into `Composition` and the FFmpeg encoder. A video placement is accepted only when a prepared renderer recipe explicitly consumes its media ID; unconsumed video and image media remain errors. The `terminal-recording` recipe maps the original global clock through the planned timeline range into source time, samples `VideoFrameCache`, and presents authentic TUI pixels through the existing terminal compositor. It does not pass video to the audio-only encoder or create a generic video layer.

Scene Plan v2 allows a scalar initial value or event target to reference one component of a stable Semantic Target plus an offset. The core validates target identity, component names, and finite offsets without interpreting the target selector. During renderer preparation, the actor's concrete recipe resolves selectors into geometry; only then are ordinary numeric Property Tracks compiled. This keeps font measurement renderer-owned while preserving pointer and highlight trajectories as inspectable general channels.

`crates/kinograph/src/timeline.rs` compiles pure animation values into deterministic scalar property tracks. Its renderer-neutral Interface drives the hero scene's panel, code-layout, code-content, focus, highlight, and pointer properties while preserving velocity across spring retargeting.

```text
Code snapshots ──> CodeTransition.sample(progress) ──┐
                                                     ├─> EditorFrame
Motion cues ─────> Spring.sample(state, target, t) ──┘       │
                                                            v
                                                 HeadlessRenderer.render
                                                            │ RGBA
                                                            v
                                                 FfmpegEncoder.write_frame
```

## Stable Code Is Independent of Rendering

`crates/kinograph/src/code.rs` owns stable line IDs, code documents, snapshots, validation, and line placement. Its Interface is `CodeTransition::compile` followed by `CodeTransition::sample`.

The Module does not know about glyphs, GPUs, colors, FFmpeg, or absolute screen coordinates. This gives the identity and layout rules locality and makes snapshot behavior testable without external systems.

The Effect Institute corpus and hand-authored lesson ports demonstrated flat stable inline identity. `CodeLine` therefore owns ordered `InlinePart` values and validated logical semantic ranges while retaining a derived flattened span view for the current renderer. The model intentionally stops before a recursive slot AST, automatic diffing, or glyph geometry.

Maximum Stability uses authored identity, not an inferred text diff. `CodeTransition::compile` still matches line IDs between two row maps. The planned editor preserves that legacy path when `snapshots` is empty. A timed `EditorSnapshotPlan` schedule instead lowers through `EditorRecipePlan::snapshot_channels` into private per-line y and opacity channels. Lines absent from both endpoints remain available between them; equal-time snapshots coalesce before layout, and unchanged row/presence targets do not restart. Native playback includes those generated channels in ordinary destination compilation, so insertion, removal, reordering, and re-entry use the same velocity-preserving tracks as video.

`kinograph/src/editor/stability.rs` provides GPU-free `inspect_steps`, exposed by `kinograph plan steps` and the persistent server's `steps` command. It reports before/after text, changed-part markers, retained-line movement, unsettled or partial holds, and heuristic common-text warnings for exchanged parts or replaced lines. It never assigns identity automatically. The ordinary Scene Plan JSON diff remains a structural plan diff, not an animation-stability analysis.

## Motion Carries Position and Velocity

`crates/kinograph/src/motion.rs` owns the analytic damped-spring equation. Its Interface accepts an initial motion state, target, and arbitrary elapsed time.

This Interface provides leverage beyond easing: deterministic out-of-order sampling and momentum-preserving interruptions use the same Implementation. Relative Rust `Animation` values and explicit-time Scene Plan events both compile through `crates/kinograph/src/timeline.rs` into the same segment representation.

Each compiled spring has a deterministic settling time. Underdamped motion uses decaying amplitude bounds; critical damping checks the remaining extrema of its exponential-polynomial solution. After that time the segment stays exactly at rest, even when another channel resumes the local clock. The event boundary retains the full initial state. Relative choreography's existing unit-distance advance threshold is unchanged; it does not guarantee physical rest for every displacement.

## Composition Owns Cross-Media Time

`crates/kinograph/src/composition.rs` keeps immutable source assets separate from their uses in an edit. A `Clip` selects an exact source range; compiling a `Composition` places that range on the output timeline without modifying the asset.

Composition supplies sequence, parallel, delay, hold, and named cue ranges across visual `Motion`, semantic annotations, stable Task state and pose changes, and timed media. It distinguishes transcript-bearing script clips from accompanying layer clips such as sound effects, music, and B-roll. Cue-local parallel composition keeps an annotation and its sound synchronized as one authored event without coupling renderer recipes to audio assets. This mirrors the real behavioral difference exposed by transcript-led editors without introducing a graphical editor or media decoder.

Media time is stored as integer nanoseconds. Conversion to floating-point seconds happens only when visual motion is lowered into scalar trajectories, so repeated source-range edits retain exact boundaries.

Still images remain stable scene actors rather than pretending to be time-based clips. A compiled scene retains each image asset and its transform property IDs; actual image composition remains renderer work for the first image-backed scene.

`crates/kinograph-render/src/video.rs` is the narrow input-video boundary demonstrated by the OpenCode command-hot-reload scene. FFmpeg decodes a checked-in H.264 terminal recording into an ignored seekable RGBA cache under `target/`; fixed-size frame offsets then provide deterministic arbitrary-time sampling without codec bindings or retaining the decoded recording in memory. The cache is regenerated when the immutable source changes.

## Transcript Cues Drive Choreography

`crates/kinograph/src/transcript.rs` ingests word timing sidecars and resolves exact word occurrences into cue ranges. The `effect-shows-errors`, `promises-only-happy-path`, and `effect-is-a-description` ports demonstrate the intended seam: original narration is a script clip, while published word timings schedule ordinary code, Task, and pointer motions through `Cue::at` on the same composition clock.

Transcript parsing does not understand code, actors, or rendering. It only connects semantic words to the shared media clock.

## Rendering Is One Concrete Adapter

`crates/kinograph-render/src/render.rs` is the concrete `wgpu` and `cosmic-text` Adapter. `HeadlessRenderer::render_shapes` renders transparent editor-local geometry into tightly packed RGBA pixels. Cached stable-line sprites, pointers, and annotations are added to that flat editor surface before the shared card compositor presents it.

The Adapter also resolves semantic token targets from `cosmic-text` glyph cluster hitboxes. Token highlights and the pointer consume those measured bounds; choreography does not estimate monospace character widths or hardcode target coordinates.

Editor panel translation, three-axis rotation, and scale are sampled properties. The editor shader remains flat and transparent; `render/ui/card.rs` is the sole perspective implementation for both editor and recorded-video surfaces. Depth-weighted Gaussian sampling softens the near edge during the opening pose, while increased entrance shutter sampling keeps fast perspective motion continuous.

Prepared Scene Plans derive an exact visual key from sampled motion position and velocity, prior pointer-motion state, discrete State Track values, terminal source-frame identity, and recipe-owned internal tracks. Shutter samples with the same key render once and contribute their multiplicity to linear-light accumulation. Concrete renderer-owned scenes retain unique time keys because their private recipes may depend directly on time. This optimization preserves arbitrary-time semantics and cannot collapse active motion merely because neighboring encoded frames happen to look similar.

SVG assets are parsed and rasterized once into reusable cached sprites, then transformed and composited for each temporal sample. The pointer uses the exact filled Phosphor `HandPointingIcon` path selected by default in `effect-institute`; the same path supports future SVG actors without adding asset-specific shader geometry.

SVG sprites are rasterized at four times their display resolution and coverage-sampled during rotation. The compositor also supports animated scale, opacity, and blur, which the pointer uses for its Effect Institute-style entrance.

Pointer translation uses mildly underdamped scalar property tracks; rotation is derived from sampled velocity and acceleration so the hand leans against acceleration and follows through while decelerating. The complete transform remains deterministic and participates in temporal accumulation.

Stable code lines can be split into cached stable and variable sprites. An inline reveal opens or closes a variable span's layout width while opacity and blur resolve; every following stable or variable span derives its position from the sampled widths before it. Multiple non-overlapping reveals can exchange Effect and function alternatives horizontally while `const getTime`, ` = `, `getTime`, and `)` retain identity. This is the concrete maximum-stability seam demonstrated by Effect Institute ports and remains narrower than a general recursive slot AST.

Editor text and bright source regions share fractional glyph sampling. Premultiplied bilinear sampling preserves subpixel translation; source-range coverage clips fractional reveal columns once, without double-attenuating raster borders. Weighted blur taps move continuously rather than rounding their offsets. Intersecting line pixels clip against the viewport instead of dropping a complete line at its boundary. Planned text actors use this path too. This intentionally changes preview and export pixels; final-window smooth/pixelated filtering remains separate.

The lesson port extends the compositor to multiple non-overlapping reveals on one stable line. Focus ranges and token highlights carry independent vertical geometry, so cursor-only cues do not accidentally move or resize focus. Semantic annotations currently include a CPU-composited error squiggle and short celebration effects; both participate in temporal sampling.

`crates/kinograph-render/src/render/effects/` owns the concrete pixel implementations for celebration annotations. Prismatic bloom and focus pulse share one small frame interface but keep their particle and halo recipes in separate files. `render.rs` only translates resolved target geometry into canvas coordinates and dispatches the selected closed recipe; adding another demonstrated built-in effect does not enlarge the renderer adapter or require a plugin interface.

`crates/kinograph-render/src/render/task.rs` owns the concrete Effect Task visual recipe demonstrated by the `visual-effects` lesson: compressed running nodes, energy sweeps, state flashes, pulses, icons, error bubbles, and labels. The DSL and composition retain stable Task identity and semantic state changes; compiled Task frames also carry the previous state and its completed duration so the renderer can overlap outgoing content and preserve running phase without mutable frame history. This renderer alone owns the current 128-pixel presentation. The same recipe can render a complete Task scene or composite Task actors over existing editor pixels, as required by `effect-is-a-description`. All moving pixel layers share one fractional transform, rounded signed-distance edge, and analytic coverage so the body, sweep, border, pulse, and glow remain one coherent material. Result content keeps its natural transform but is analytically masked by the current sampled rounded body after text blur, preventing spring intermediates from leaking outside the Task without scaling content to fit. A short container-level entrance blur applies to the assembled node while ordinary shutter sampling supplies motion blur from actual movement.

`crates/kinograph-render/src/render/terminal.rs` owns the concrete OpenCode terminal recipe: source-frame selection, a short split-screen command-file editor, and the missile payoff. The source recording supplies authentic TUI pixels; Kinograph supplies whole-card camera movement, the external file-write explanation, impact defocus and shake, and foreground effects. Rounded material, border, shadow, perspective, and depth blur are not terminal concepts; the recipe delegates them to the shared card compositor. This is intentionally not a terminal emulator, ANSI parser, generic video layer, or particle plugin.

`crates/kinograph-render/src/render/deployment_queue.rs` is the first state-driven simulated UI Surface. One typed `deployment-queue` actor owns an ordered catalog and timestamped UI Snapshots rather than one actor per row. During preparation, `plan_runtime/deployment_queue.rs` validates the opaque recipe, computes card-local row targets through the private flow layout, and compiles stable row position, presence, progress, phase-feedback, and attention tracks. `plan_runtime/keyed_layout.rs` is a private generic Implementation behind that recipe; interrupted reorders preserve velocity through the ordinary Timeline compiler, exiting items retain their previous phase until presence settles, and equal-time zero-duration snapshots do not create phantom identity. The recipe contributes its internal sampled values to shutter deduplication and paints a fresh fixed-canvas dashboard for every Temporal Sample without entities, events, retained widgets, or wall-clock animation.

`scenes/opencode-session-tool` combines planned video, layered SFX, continuous card motion, discrete recording state, and named cues through the process seam. Its immutable source recording aligns a real Vim session on the left with one already-running OpenCode v2 client on the right; the concrete terminal recipe preserves those source pixels and their aspect ratio while ordinary `text` actors carry explanatory overlays. The 20.5-second rapid-fire artifact demonstrates live command, agent, project-skill, reference, model, permission, ambient-instruction, and local-plugin generation changes without restarting the OpenCode service, client, or session.

`scenes/deployment-queue` is the first state-driven simulated UI Scene Program. Its typed recipe data owns product copy and a stable service catalog; seven authored state changes drive deployment phases, progress, insertion, failure focus, retry, and final health without authored row coordinates or per-frame calculations. The concrete proof keeps service order stable because focus and color communicate failure priority without a gratuitous full-width row crossing. A phase replacement retains the immediately preceding complete UI Snapshot, keeps row geometry continuous, interpolates color and chip width, sequences old and new text, and transitions aggregate health from the same snapshot pair. Aggregate progress is monotonic, and a stagger is bounded by the next snapshot so delayed presentation cannot outlive newer semantic state.

`scenes/quark-before-after` is a compact narrated tutorial contrasting Solid Store reconciliation with explicit keyed identity. It uses the existing editor, text, and Script Clip recipes rather than introducing a reactive-system visualization or another renderer abstraction. The scene also demonstrates the planned editor's shared perspective-card entrance and multiple channel-driven Inline Reveals: one new Stable Line enters before opposing variable parts exchange inside otherwise stable lines, then the update call changes on its own narration cue.

`crates/kinograph-render/src/render/ui.rs` is a private, GPUI-inspired immediate-mode vocabulary for renderer-owned pixel interfaces. Immutable `Bounds` values split, inset, and align child regions; axis-aware `Flow` and equal tracks derive row and column placement from extents and gaps. `render/ui/card.rs` adds nested rounded clips, fills, strokes, packed or strided RGBA sources, fit modes, card-local overlays, and projected cards with one material, border, shadow, surface blur, and depth-dependent near-edge blur. Draw order is z-order and closures provide local composition without retaining public nodes. Flattened editor pixels, decoded terminal-video pixels, and the composed deployment dashboard all enter through this Module. A direct borrowed-source operation preserves the same presentation semantics without first rasterizing a second full-card intermediate; the closure path remains available when a card needs multiple composed layers. The Module intentionally stops before an element tree, flexbox engine, event model, retained widgets, renderer trait, or public UI framework.

The Adapter keeps these details private:

- headless Metal adapter and device creation
- WGSL pipeline and uniforms
- CommitMono loading and raster-sprite caching by stable line ID
- texture-to-buffer row alignment
- asynchronous mapping and GPU polling
- the flat dark editor treatment

There is no renderer trait. One Adapter is a hypothetical seam; a second backend would make it real.

## Encoding Is One Concrete Adapter

`plan_runtime/delivery.rs` owns PNG and MP4 delivery separately from `PreparedPlan` preparation and sampling. Video exports keep the authored timeline, full visual quality, shutter samples, and original audio placements.

`kinograph/src/playback.rs` derives numeric step destinations from a renderer-prepared Timeline, after semantic geometry has resolved. Next, Previous, First, and Last append only changed channel targets through the shared Timeline compiler. Each spring therefore inherits position and velocity, including mid-flight reversals; unchanged destinations do not restart motion. Per-channel motion profiles come from the destination's latest authored spring (or its first spring before any event; set-only channels use a 0.4-second zero-bounce default). Replay alone resets to the entry pose. The local clock freezes on pause or once all channels settle, without retiming the authored video. Immutable `Arc<Timeline>` revisions make sampling history-independent even while input creates a newer revision.

`plan_runtime/presentation.rs` owns winit lifecycle, slide/step navigation, full screen, letterboxed resizing, and smooth/pixelated display filtering. `presentation/worker.rs` retains the deck's prepared scenes, fonts, and GPU. At most one render is in flight; requests and results carry slide identity and immutable timeline revisions, so switching slides cannot display a stale result from another scene. Each slide retains its selected step and paused local clock while inactive. Explicit pause stays paused on return; previously running motion resumes. Held/paused scenes sleep unless a planned Task requests ambient clock advancement. Generic State Channels and recorded media remain unsupported by interruptible playback; video support is unchanged.

Semantic coordinates retain a numeric expanded-layout baseline plus private companion weights compiled by `plan_runtime/attachments.rs`. Sampling applies each weight to the difference between expanded and visible target geometry, including product-rule velocity. Target measurements use the same span partitions and shaping as inline painting; weight tolerances are normalized to the measured coordinate extent. Companion tracks participate in native retargeting, preserving continuity when switching between moving targets and literal coordinates. Line-layout and reveal drivers must use literals: semantic feedback into their own geometry is rejected. These are prepared renderer tracks, not new public Scene Plan fields.

`kinograph/src/task.rs` describes planned Task states, and `plan_runtime/task.rs` lowers them into continuous geometry, color weights, activity, and independent content/bubble channels. `TaskVisualFrame` reuses the existing Rust Task material, icons, result clipping, error bubbles, jitter, and energy sweep instead of importing a browser runtime. The source Pixi profiles remain distinct: 0.2-second/bounce-0.5 height, 0.35-second/bounce-0.35 width, approximately 0.167-second icon opacity/scale/blur, and 0.25-second/bounce-0.4 result scale with 0.15-second deblur. State content can reverse mid-transition through the shared Timeline. The activity track keeps the local clock and visual sample key advancing; pause, slide departure, and reduced motion freeze it. This is a scoped native adaptation of Effect Institute's Pixi blocks, not a general implementation of browser component state or actual Effect execution.

`render/task/content.rs` contains sampled `ContentPose`, `BubblePose`, and `TaskContentFrame` values. It does not remap a shared state-progress spring through a second easing or delayed visibility window. Preparation compiles `content.<state>.*` and `bubble.<state>.*` channels, including the source's independent bubble rise/fade/deblur. These channels enter native Playback like any other trajectory; no edge-triggered timer or direction branch can reset a reversing pose. The symbol rotates independently while its clip remains aligned with the body. Error text, background, and tail share one cached bubble sprite so its entrance transforms the whole surface. An outgoing bubble is not culled by its faster icon fade. Unchanged channels keep content still. No public Scene Plan fields or generic transition framework are added.

The source's instantaneous running toggle becomes a short 0.06-second continuity ramp, without waiting for content to disappear. Retained symbols use a symmetric 0.7 hidden scale instead of the source's new-icon mount resets and 0.9 exit scale; selective symbol rotation remains an intentional native flourish. Results retain a 0.5 hidden scale. These adaptations preserve arbitrary-step continuity, so the timing tests claim source spring-curve parity, not complete source-renderer pixel equivalence. `tests/fixtures/effect-task-timing.json` comes from the source project's pinned Motion DOM 12.42.2 generator and tests actual compiled channel samples, including overshoot and separate result deblur.

Transformed sprites use denser 5×5 binomial filter taps: the larger content defocus exposed visible displaced copies with the former 3×3 taps, particularly in error-bubble text. Tap positions still vary continuously, and the unblurred identity case retains direct bilinear sampling. This improves a sampled optical effect, not the trajectory or the export shutter model.

`presentation/gpu.rs` owns the final native surface, independently of scene recipes. It uploads changed RGBA frames to one sRGB texture and draws a letterboxed triangle with linear or nearest filtering. FIFO presentation requests one-frame latency. Resizing or switching filters reuses uploaded pixels; lost/outdated surfaces are rebuilt and zero-sized/occluded windows defer presentation. This replaces the measured CPU scaling/softbuffer bottleneck without migrating choreography or code layout to another renderer. `presentation/benchmark.rs` measures one warmup plus nine native interruption rounds; `perf/native-playback.md` records the comparison and its limits.

Native sampling follows the current monitor's reported millihertz refresh rate (60 Hz fallback), with an explicit `--fps` override independent of video timing. Moving, resizing, or refocusing the window rechecks the rate. Neither setting changes FIFO synchronization or the one-in-flight worker bound. `--benchmark` records display rate and acquisition wait; `--benchmark-gpu` additionally waits for each submitted draw to finish and reports scene sampling plus completed upload/draw work, excluding drawable acquisition. That diagnostic wait is never enabled during ordinary playback. Display refresh, rendering headroom, and verified scanout FPS are distinct measurements.

The native preview is a deliberate quality profile, not a second choreography implementation. A neutral editor with no visible pointer or annotation reuses static composed chrome and paints code directly, omitting export's final optical resampling. Dynamic focus/highlight overlays use the same WGSL recipe in a chrome-free pass, avoiding an optical-card rebuild whenever an attached highlight moves. Unsupported transforms/effects fall back to the full renderer. `--full-quality` disables this shortcut; export never enables it. Pixel equality is required across sampling order within a profile, not between profiles. Higher-DPI glyph rasterization, presentation audio, and live source reloading remain future work.

The cheap overlay pass is limited to bounds safely inside the card body; overlays that can overlap title/border pixels or escape its clip use the full path. Two filename-keyed chrome images are retained, and each slide's initial resources are warmed before the native window opens. Frame deadlines retain their phase across late wakes rather than drifting relative to the previous request. Missed slots are skipped, not queued.

`crates/kinograph-render/src/encode.rs` owns the FFmpeg process, raw-frame protocol, audio placement filters, argument construction, and exit validation. The Interface accepts tightly packed RGBA frames plus compiled audio media placements. FFmpeg trims immutable source ranges, applies each clip's non-destructive gain, shifts clips onto the composition clock, mixes overlapping layers through a peak limiter, and encodes AAC beside H.264.

FFmpeg remains a subprocess because it avoids unsafe bindings and codec linkage while preserving access to the installed encoder set. A second encoder is not currently justified.

## Scene Modules Own Choreography

`crates/kinograph-render/src/main.rs` parses the command, selects an output, and dispatches to one concrete Module under `crates/kinograph-render/src/scenes/`. Each scene Module keeps its assets, documents, snapshots, semantic targets, choreography, and sample rendering local behind one `render(output)` interface. `crates/kinograph-render/src/scenes/mod.rs` contains only mechanics shared by demonstrated scenes: delivery dimensions, temporal accumulation, semantic target measurement, pointer sampling, and styled-span construction.

This is a locality seam, not a scene framework: there is no scene trait, registry, or generic lifecycle. Shared authoring operations should move into the DSL only when repeated usage reveals a deeper interface.

The hero is the first complete exception to direct scene-module execution. `scenes/hero` is a lightweight Rust Scene Program containing its stable editor document, logical semantic ranges, actors, channels, and exact choreography. It emits `scenes/hero/hero.plan.json`; a byte-equality test keeps that generated canonical plan synchronized with Rust source. The default renderer embeds the plan for command compatibility, resolves its editor targets through `crates/kinograph-render/src/plan_runtime/editor.rs`, and renders through the shared persistent-plan path. The former direct `scenes/hero.rs` implementation was removed after the full 300-frame H.264 artifact matched byte-for-byte.

`crates/kinograph-render/src/scenes/effect_institute.rs` is the private exception for a demonstrated external corpus: 30 published sections across the first two Effect Institute chapters. It reads pinned, checked-in compiled lesson artifacts rather than accepting user-authored JSON. Stable template line, part, and version IDs lower into ordinary Kinograph line/opacity/part property tracks; published step times retarget those tracks with the same line and inline motion contracts used by hand-authored ports. Component snapshots lower into the existing concrete Task recipe or one closed section-local presentation. The adapter does not add a public JSON authoring boundary or generic scene graph.

Published cursors, token highlights, focus regions, and long-section camera offsets compile through the same explicit-time Timeline segment compiler used by Scene Plans rather than a second analytic track implementation. Their retargets preserve velocity at rapid cues, while hidden component actors stay mounted so mode changes use Task entry/exit motion instead of changing row identity. Imported component snapshots and authored Tasks both use the generic `StateTrack<TaskState>` implementation for previous value, state age, and visibility intervals. The imported editor clips and softly fades camera-edge content to its code viewport.

A stitched chapter is one encoder job over exact section narration placements, but each section keeps a zero-based local clock and independent actor namespace. One exact `TimeRange` interval schedule derives both media composition and visual selection; lesson IDs compile into global named cues. Unrelated sections do not interpolate actor identity or momentum across boundaries.

## The Rust DSL Produces Pure Values

`crates/kinograph/src/dsl.rs` is the existing typed authoring boundary exported through `crates/kinograph/src/lib.rs`. Authors compose property `Motion`, semantic `Annotation` values, Task state and pose changes, and media in a `Composition`. `crates/kinograph/src/author.rs` adds stable handles for Scene Programs that emit versioned plans. Both frontends lower continuous changes into the same Timeline compiler and discrete values into generic State Tracks.

`CodeEdit` is the first authoring operation extracted from repeated scene usage. It owns the layout/content tracks for one coordinated structural edit, supplies their initial values, returns enter/exit Motion, and samples a `CodeTransition` against a compiled Scene. It does not absorb `CodeTransition` or hide intentionally staggered tracks such as the hero's separate layout and content cues. `Cue::at` similarly centralizes exact cue-start placement for any composable leaf without teaching cues about Motion, Tasks, annotations, or media roles.

Scalar targets may remain semantic while authoring. The in-process DSL uses `TextTarget` and `Scalar`; Scene Plans use stable `SemanticTargetPlan` declarations and `ScalarPlan` target references. Both request target edges, widths, centers, line positions, or attached offsets and resolve geometry once before compiling the same numeric Timeline.

The hero and lesson ports are concrete clients of these APIs. Scene Programs may serialize generated Scene Plans as JSON for the process protocol, but JSON and TypeScript are not source authoring languages.

## Current Stack Decisions

- [`wgpu 30`](https://github.com/gfx-rs/wgpu) is the headless GPU substrate. Rendering targets an offscreen texture without a window or surface.
- [`cosmic-text`](https://github.com/pop-os/cosmic-text) shapes and rasterizes CommitMono lines into stable cached sprites.
- [`glyphon`](https://github.com/grovesNL/glyphon) was evaluated and removed: repeatedly preparing a dynamic GPU glyph atlas across temporal samples can invalidate glyph coordinates during atlas growth. Cached line sprites fit Kinograph's stable-identity model better.
- WGSL remains the shader language because it is native to wgpu and translated by Naga.
- An FFmpeg subprocess handles H.264 encoding. [`ffmpeg-next`](https://github.com/zmwangx/rust-ffmpeg) is maintenance-only and adds an unnecessary FFI seam.
- [`Vello`](https://github.com/linebender/vello) remains deferred because its API and wgpu compatibility are still moving. `lyon` is the likely addition if authored vector paths become necessary.
- The current RGBA8 render target is sufficient for the visual prototype. Temporal samples are decoded to linear light before CPU accumulation and converted back to sRGB once per output frame, avoiding dark gamma-space motion trails. A production compositor should render and accumulate directly in linear `Rgba16Float`, then tone-map into the delivery color space.

## Explicit Non-Abstractions

The prototype does not have a generic scene graph, renderer trait, plugin interface, render graph, dynamically loaded Rust library, or recursive slot AST. The two-crate workspace exists only to keep lightweight Scene Programs independent from the heavyweight persistent renderer. Further package seams require another demonstrated compilation or deployment need.
