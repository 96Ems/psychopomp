# Prototype Architecture

Psychopomp is organized around a small set of Modules whose Interfaces correspond to demonstrated change seams: code identity, media composition, motion trajectories, scalar tracks, rendering, and encoding.

Video export and native interactive presentation share Scene Plan preparation, scalar tracks, and visual recipes. Winit owns the native window and input; a wgpu surface displays Rust-rendered RGBA pixels. These supported paths have no browser playback layer or second animation implementation.

`experiments/browser-grid-prototype` is an isolated, throwaway browser feasibility
probe, not another supported crate boundary. It compiles the existing grid/diagram
layout, shaders, and Playback for WASM/WebGPU with build-time baked labels and GPU-only
canvas delivery. It does not port the complete renderer, native slide text, live
code typography, or video export. Its own notes record near-native pixel evidence,
client memory/timing costs and remaining compatibility/accessibility work.
The browser host now acquires a matching RGBA canvas texture before each sample,
so shared grid passes target its sRGB view directly instead of a redundant full-frame
texture/blit. Native readback/export delivery is unchanged. Grid label atlases in
both hosts use R8 coverage; theme color stays in uniforms. `perf/browser-grid.md`
records the fixed-quality autoresearch and rejected alternatives.

## Scene Programs And Rendering Compile Separately

The workspace has one demonstrated package seam. `crates/psychopomp` is a lightweight library containing authoring values, versioned Scene Plans, validation, exact composition time, continuous Property Tracks, discrete State Tracks, and stable code identity. `crates/psychopomp-render` contains `wgpu`, `cosmic-text`, video decoding, FFmpeg encoding, built-in renderer recipes, and the CLI.

```text
Rust Scene Program -> Scene Plan -> persistent psychopomp-render process
```

A Scene Program remains ordinary Rust and may perform arbitrary calculations before emitting a plan. The Scene Plan is compiled output rather than a replacement authoring language. Its renderer-independent Interface contains stable actors, continuous channels, state channels, exact cues, and media placements. Renderer Recipe payloads remain opaque to the core and are interpreted only by concrete adapters in `psychopomp-render`.

`crates/psychopomp-render/src/plan_runtime.rs` validates and inspects plans without initializing a GPU, renders exact global Render Windows, and provides a newline-delimited JSON server that retains the GPU device and font caches across requests. Intersecting media is trimmed and rebased to output zero while visual sampling remains on the original global clock. Explicit event selection uses a `f64` seconds clock derived from serialized integer nanoseconds; scalar spring positions and velocities remain `f32`.

`plan_runtime/preflight.rs` owns decoded recipe inputs, exclusive root selection,
references, and native eligibility before resource creation. Resource preparation
consumes those inputs to construct one complete `PreparedPlan`; it does not install
independent optional roots afterward. `CompiledPlan` keeps numeric/state/audio
compilation separate from resource ownership. Plain text and bounded Markdown are
parsed once, not interpreted from actor JSON during sampling. Typed current-state
projections consider only observable values; raw State Tracks retain distinct
equal-time predecessors for snapshot semantics and exact visual keys.
State Tracks keep predecessor clones rather than borrowing another segment's
current payload: `Clone` may deliberately isolate interior-mutable values.

Shared rules have narrow owners rather than a recipe registry:

- `psychopomp::plan::compile_channels` lowers scalar events exactly. Its callers
  choose channels, resolve semantic scalars, and retain diagnostic context.
- `destination_channel` and `effective_snapshots` opt snapshot recipes into final
  equal-time destinations and unchanged-target suppression. Raw continuous events
  and distinct State Track transitions keep their authored ordering.
- `plan_runtime/generated.rs` reserves generated channel IDs and actor/property
  pairs. Task x/y is the sole explicit authored override, not last-writer-wins.

Planned audio and visual media share the same exact source and timeline ranges but have different concrete consumers. Audio lowers into `Composition` and the FFmpeg encoder. A video placement is accepted only when a prepared renderer recipe explicitly consumes its media ID; unconsumed video and image media remain errors. The `terminal-recording` recipe maps the original global clock through the planned timeline range into source time, samples `VideoFrameCache`, and presents authentic TUI pixels through the existing terminal compositor. It does not pass video to the audio-only encoder or create a generic video layer.

Scene Plan v2 allows a scalar initial value or event target to reference one component of a stable Semantic Target plus an offset. The core validates target identity, component names, and finite offsets without interpreting the target selector. During renderer preparation, the actor's concrete recipe resolves selectors into geometry; only then are ordinary numeric Property Tracks compiled. This keeps font measurement renderer-owned while preserving pointer and highlight trajectories as inspectable general channels.

`crates/psychopomp/src/timeline.rs` compiles pure animation values into deterministic scalar property tracks. Its renderer-neutral Interface drives the hero scene's panel, code-layout, code-content, focus, highlight, and pointer properties while preserving velocity across spring retargeting.

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

`crates/psychopomp/src/code.rs` owns stable line IDs, code documents, snapshots, validation, and line placement. Its Interface is `CodeTransition::compile` followed by `CodeTransition::sample`.

The Module does not know about glyphs, GPUs, colors, FFmpeg, or absolute screen coordinates. This gives the identity and layout rules locality and makes snapshot behavior testable without external systems.

The Effect Institute corpus and hand-authored lesson ports demonstrated flat stable inline identity. `CodeLine` therefore owns ordered `InlinePart` values and validated logical semantic ranges while retaining a derived flattened span view for the current renderer. The model intentionally stops before a recursive slot AST, automatic diffing, or glyph geometry.

Maximum Stability uses authored identity, not an inferred text diff. `CodeTransition::compile` still matches line IDs between two row maps. The planned editor preserves that legacy path when `snapshots` is empty. A timed `EditorSnapshotPlan` schedule instead lowers through `CompiledEditor::snapshot_channels` into private per-line y and opacity channels. Lines absent from both endpoints remain available between them; equal-time snapshots coalesce before layout, and unchanged row/presence targets do not restart. Native playback includes those generated channels in ordinary destination compilation, so insertion, removal, reordering, and re-entry use the same velocity-preserving tracks as video.

`psychopomp::editor::compiled` owns the validated catalog, legacy/keyed placement,
and reveal span/part ranges used by both inspection and renderer preparation.
Keyed sampling does not fabricate a two-snapshot transition.

`psychopomp/src/editor/stability.rs` provides GPU-free `inspect_steps`, exposed by `psychopomp plan steps` and the persistent server's `steps` command. It reports before/after text, changed-part markers, retained-line movement, unsettled or partial holds, and heuristic common-text warnings for exchanged parts or replaced lines. It never assigns identity automatically. The ordinary Scene Plan JSON diff remains a structural plan diff, not an animation-stability analysis.

## Motion Carries Position and Velocity

`crates/psychopomp/src/motion.rs` owns the analytic damped-spring equation. Its Interface accepts an initial motion state, target, and arbitrary elapsed time.

This Interface provides leverage beyond easing: deterministic out-of-order sampling and momentum-preserving interruptions use the same Implementation. Relative Rust `Animation` values and explicit-time Scene Plan events both compile through `crates/psychopomp/src/timeline.rs` into the same segment representation.

Each compiled spring has a deterministic settling time. Underdamped motion uses decaying amplitude bounds; critical damping checks the remaining extrema of its exponential-polynomial solution. After that time the segment stays exactly at rest, even when another channel resumes the local clock. The event boundary retains the full initial state. Relative choreography's existing unit-distance advance threshold is unchanged; it does not guarantee physical rest for every displacement.

## Composition Owns Cross-Media Time

`crates/psychopomp/src/composition.rs` keeps immutable source assets separate from their uses in an edit. A `Clip` selects an exact source range; compiling a `Composition` places that range on the output timeline without modifying the asset.

Composition supplies sequence, parallel, delay, hold, and named cues across visual `Motion`, semantic annotations, stable Task state and pose changes, and timed media. It distinguishes transcript-bearing script clips from accompanying layer clips such as sound effects, music, and B-roll. Cue-local parallel composition keeps an annotation and its sound synchronized as one authored event without coupling renderer recipes to audio assets. This mirrors the real behavioral difference exposed by transcript-led editors without introducing a graphical editor or media decoder.

Media time is stored as integer nanoseconds. Conversion to floating-point seconds happens only when visual motion is lowered into scalar trajectories, so repeated source-range edits retain exact boundaries.

`crates/psychopomp-render/src/video.rs` is the narrow input-video boundary demonstrated by the OpenCode command-hot-reload scene. FFmpeg decodes a checked-in H.264 terminal recording into an ignored seekable RGBA cache under `target/`; fixed-size frame offsets then provide deterministic arbitrary-time sampling without codec bindings or retaining the decoded recording in memory. The cache is regenerated when the immutable source changes.

The decoder receives null stdin, never the persistent server's request stream.
A failed seek/read invalidates the in-memory frame identity before it can expose
partially overwritten bytes as a cache hit.

## Transcript Cues Drive Choreography

`crates/psychopomp/src/transcript.rs` ingests word timing sidecars and resolves exact word occurrences into cue ranges. The `effect-shows-errors`, `promises-only-happy-path`, and `effect-is-a-description` ports demonstrate the intended seam: original narration is a script clip, while published word timings schedule ordinary code, Task, and pointer motions through `Cue::at` on the same composition clock.

Transcript parsing does not understand code, actors, or rendering. It only connects semantic words to the shared media clock.

`Transcript::phrase` and `phrase_after` match consecutive normalized words (case,
punctuation, and number words versus digits are ignored). `scripts/narrate.ts`
produces clips, loudness-normalized MP3s, Whisper word timings, and a manifest of
exact durations; its `--draft` mode uses macOS `say` so a scene can be timed before
the final voice exists. `psychopomp::narration` loads that manifest and places each
clip as a Script Clip whose phrase lookups return plan-clock times. The
`pr-walkthrough` Scene Program keys every reveal to a
phrase and fails with the clip and phrase when narration no longer says it.

## Rendering Is One Concrete Adapter

`render/theme.rs` owns the presentation paint tokens. Native T/Shift+T selects a
theme and `presentation/preferences.rs` atomically saves it under the user's
configuration directory. Worker requests, cached frames, and results include the
theme as well as the grid palette, so stale appearance cannot win a race. Theme
changes clear colored editor resources, but never recompile a Timeline or change
glyph metrics. Grid clear/material/ink colors are GPU uniforms; text colors are
resolved before coverage blending. Existing neutral RGB typography and known
syntax/showroom colors have a compatibility mapping; other literal art/status
colors remain authored. This is not a final-frame color filter. Native appearance
does not mutate plans. `plan frame` and `plan render --theme NAME` use the same
renderer with an explicit theme; omitted export themes remain Original.

The provisional `prototype-rich-text` adapter parses a bounded Markdown subset
with `pulldown-cmark` and shapes bold/italic/monospace runs with `cosmic-text`.
Paragraph wrapping is measured once, with a lazy current-theme sprite cache.
Inline code backgrounds, list markers, quote rules, links, and strikethroughs share
those measurements. Actor x/y/opacity/reveal/blur and `block.N.opacity`/`block.N.y`
are ordinary scalar channels. A stationary vertical mask supports rising headers.
Rich-text fades are sharp by default; `fade_blur` is an explicit optical opt-in
for titles, independent of the channel's motion profile. The showroom uses
160 ms zero-bounce prose fades and retains the approved 400 ms header rise.
HTML and images are rejected; no network, browser, Markdown-table layout, or
dynamic document diffing is introduced. Fenced code is monospaced, not highlighted.

`prototype-width-text` reuses Typeset identity, measured advances, and connector
anchors with visual-types-style width/blur disclosure rather than a uniform fade.
Optional `TextPart.spans` reuses editor `StyledSpan` roles inside a single measured
and animated part. Token style no longer has to equal animation-part identity;
preflight rejects runs whose concatenation disagrees with the backing text.
`prototype-venn` samples two rounded boundaries and unions stroke coverage; hatch
coverage is their actual signed-distance intersection, including the square morph.
Callout endpoints derive from the sampled boundaries and stay outside both sets.
Both remain provisional overlays. `scenes/component-prototypes --slideshow`
combines these with rich text, full-divider tables, and the existing composition.

`prototype-header` adds a bounded bold single-line entrance, optionally partitioned
into word regions of the **same shaped line**. Split boundaries fall in whitespace;
staggering changes only vertical pose, not word placement or text shaping. A mirror
sprite shares the sampled pose and reflects across the fixed reveal edge. Its
below-edge clip and one-sided linear alpha fade never paint over other actors.
Reflection strength, depth, and gap are presentation values, not independent timers.

`plan_runtime/header.rs` compiles word entrances into reserved `__header.*` tracks
and supplies opt-in `Playback::with_start_delays` metadata. The core's `StartDelay`
contains a resting source, destination, and duration. Only matching resting
entrances wait; in-flight redirections begin immediately. The shared
`timeline::retarget::RetargetSchedule` records event ownership and removes superseded
future starts only for changed channels.
Unchanged destinations retain pending starts, executed history remains immutable,
and pause/replay/reduced-motion use the same local clock. The authored header
compiler uses that same schedule for rapid authored events. Authored nanosecond
arithmetic and native seconds arithmetic remain distinct, and rejected batches
leave the previous writes, targets and Timeline revision intact. Existing
recipes opt into no delays and keep their previous behavior.

`component_prototype` in the lightweight crate, plan runtime, and renderer holds
the provisional Typeset / Collection / Connector trials. Typeset measures authored
inline parts; Collection measures a finite keyed row/column catalog; both lower
snapshot changes to reserved `__component.*` scalar tracks. Connectors reference
those recipes' item IDs and derive Bezier paths from their currently sampled
visible bounds, rather than independently springing endpoints. Stroke segments
union analytic coverage before alpha composition. These are foreground overlays,
not additional roots, and do not yet extend editor Semantic Targets. Their
font/anchor/payload interfaces are intentionally experimental; the native
`scenes/component-prototypes` showroom is the approval surface.
Fading prototype glyphs use the existing fractional text-blur kernel with a
sampling offset of `4 * (1 - opacity)` output pixels. This optical pose follows
the same fade in either direction, without changing the scalar timeline or
measured bounds. Fully present glyphs use the identical sharp path.

The provisional `prototype-diagram` is a finite box-and-wire root, demonstrated
by `scenes/opencode-architecture`'s Daemon / merge adaptation. `plan_runtime/diagram.rs`
validates node/link identities, allowed scalar properties, bounded text/layout,
and recipe-owned resting Start Delays. `render/diagram.rs` resolves ports from
sampled box bounds and packs scalar poses for `render/diagram.wgsl`. The bare GPU
pass draws only boxes, wires, traces, halos and R8-atlas glyphs with 4× spatial AA.
Flat and equal-axis orthographic Isometric share semantic identity and layout;
the Isometric Scene Program uses an ordinary depth channel for a fast critically
damped extrusion from a fixed base, and keeps labels upright. The recipe also
accepts lift, but the current scene no longer animates a competing downward lift.
An optional `width-reveal` scalar multiplies sampled width without scaling height
or type. This separates entrance presence from the server's structural merge
width, so a skipped merge still uses the same cancellation-safe server start wait.
The Scene Program times width growth to clear the adjacent rising side faces;
this is authored choreography, not a collision solver. Upright glyphs disclose
within the sampled Isometric top face with fractional edge coverage.
`Pose::port` resolves the current
side-face midpoint, not a top-plane point. Wires compare their interpolated world
height with sampled top/side-face hits so visible endpoints are not hidden until
the lower rim. Isometric paint packets (shell, glyphs, blur and halo together) are
stably sorted by sampled solid-center depth along the view ray `(1, 1, 1)`.
Authored order breaks only equal-depth ties; Flat retains authored paint order.
This bounded whole-box painter and wire alpha attenuation are not a general
depth-sorted mesh/interpenetration solution. The frame, dotted stage,
title, caption and indicators were explicitly rejected and are no longer drawn.
Their reference metadata remains in the provisional payload; the canonical scene
no longer authors invisible chrome tracks. There is no CPU backdrop or node-image
rasterization in this recipe now.

The Scene Program owns every destination/timing; no daemon concepts, graph layout
or callbacks enter the recipe. It remains an exclusive root, not Code/Grid bounded
composition. The isolated browser probe stages these actual diagram modules and
the same Start Delay preparation. Native/export use shared RGBA readback; WASM
targets its acquired canvas directly. `perf/diagram-gpu.md` records measurements
and cross-host evidence; this is not byte-identical CPU-to-GPU pixel migration.
The earlier shared card zero-border guard and constant-texel sampler remain for
other card consumers; `perf/daemon-diagram.md` is the historical CPU experiment.

`psychopomp::value::ValueTokenPlan` is an immutable labeled tile in a finite
teaching diagram. `plan_runtime/value.rs` parses it once and samples ordinary
authored x/y/opacity/emphasis channels; it generates no extra timeline or State
Tracks. `render/value.rs` composites the tile with the existing `UiCanvas` card
coverage and cached fractional text, before foreground text annotations. Hidden
glyphs are warmed too. Border emphasis does not change text presence. This is
an overlay recipe, not another exclusive root or a general graph renderer.
`scenes/data-modeling/src/stage.rs` keeps concrete token layout, pair annotations,
and step destinations scene-local. Its second and subsequent uses are finite
sets, representation comparisons, tagged alternatives, and the nullable-pair
state table; the existing Keyed Grid still owns Cartesian-product geometry.

`psychopomp::grid` describes the immutable finite-product catalog and Grid Snapshots.
Its optional `GridStylePlan` is immutable presentation data, not another recipe.
Omitting it preserves the original JSON and pixels. `plan_runtime/grid/table.rs`
provides unequal-width, fixed-anchor table placement and conventional column
headings; the same extent and presence tracks still drive growth. Table layout
supports one depth layer in straight-on or angled view, not reassociation.
`render/grid.rs` keys its glyph atlas by labels, text style, and available width;
table text is rasterized at its chosen size and clipped to padded cells rather
than squeezed into the old symbol tile. Row rules select front-plane horizontal
edges in the existing centered-stroke pass. Background-matching fills keep opaque
depth writes but bypass face lighting. Default cube placement, typography,
lighting, and stroke behavior are unchanged.
`plan_runtime/grid.rs` validates these without a GPU, computes concrete table,
volume, and reassociation layouts, and lowers cell x/y/z, presence, emphasis,
labels, extents, slice cutaways, and camera parameters into reserved `__grid.*`
continuous channels. Equal-time snapshots
coalesce and unchanged destinations do not restart. The same prepared Timeline
drives native navigation and export; generic State Channels remain unsupported
in native presentation.

`render/grid.rs` and `render/grid.wgsl` are the first true 3D recipe: GPU-instanced
connected cells, a Depth32Float attachment, four-sample spatial AA, an orthographic
orbit, and cached two-line label textures. Detached colored blocks were rejected
in favor of the old talk's adjoining grid; a subsequent wireframe pass was made
opaque to hide rear lines. There are no gaps or per-cell scale animations.
Continuous axis extents clip growth from a leading corner, while the projection
centers the sampled visible cell bounds at canvas center on every frame. This
deliberate recentering includes fractional growth, rotation, and slice cutaways;
side headings do not skew the cell bounds. The catalog determines base scale,
with a separate continuous zoom-out for labeled reassociation groups.
An optional authored `scale` channel multiplies that conservative fit for slides
that need a larger diagram without changing the existing default. Strokes remain
output-pixel sized. A singleton depth catalog has no outside Z heading, avoiding
a misleading extra column label in a two-dimensional product.

Optional `GridCellLabelPlan` values supply primary symbols and secondary text
without replacing tuple identity. The chess example demonstrates this separation.
Cell labels share geometry and depth testing; every tuple retains its ink until
its actual face is hidden or removed. Cell ink does not fade toward the next
focused slice: doing so left a blank opaque outgoing face in front of the new
labels. Outside row/column/depth headings stay upright at projected anchors.
A selected slice clips away the other layers without dimming their still-visible
strokes. The former 18% focus-emphasis target is no longer scheduled; contrast
does not anticipate a layer's physical removal. Group headers remain text,
not backing cards. Headings use a separate premultiplied-alpha overlay after
the completed material and strokes, with no depth attachment. Their cached glyph
coverage and spatial feather blend into the existing frame; faint outgoing
headings neither overwrite cells with background-colored glyphs nor block lines.
Cell material and attached ink retain opaque depth-tested rendering.
The depth range fits sampled geometry, back faces are culled,
and a tiny stable-order depth bias resolves coplanar front faces during regrouping
without moving their geometry. This fixes a pixel regression in rapid navigation.
`render/grid/edges.rs` and its shaders draw centered 1.7-output-pixel strokes
separately from the opaque material and text. A stroke-depth prepass selects
visible edges, an RGBA16Float max-coverage pass unions coincident strokes, and a
linear-light composite resolves them over the material. Thus silhouettes,
shared grid lines, and two-face creases have the same width. Edge-local ray/box
depth and per-sample coordinates preserve coverage at grazing angles. Material
keeps catalog-order depth priority; strokes uniformly clear that priority budget
and merge shared coverage rather than choosing between neighboring opacities.
The final composite uses the existing RGBA texture and shared readback/encoding;
native presentation still uploads those pixels, rather than sharing a zero-copy
surface. Resources are lazy and only the latest grid's label atlas is retained.
This reopens procedural 3D for one concrete diagram, not arbitrary meshes,
Blender materials, a public camera graph, or a renderer abstraction.

Growth-edge disclosure is the default (`plan_runtime/grid/disclosure.rs`),
selected from the former A/B/C comparison. Disclosure tracks use ordinary native
retargeting for semantic visibility, while the feather samples existing extent
and slice bounds, with no separate reveal timer or competing growth fade.
Headings disclose in catalog order along projected X, −Y, or −Z; cell ink follows
the clipped face edge. Upright headings use a glyph-fitting aperture when their
projected cell interval is too small. The 8-output-pixel linear ramp affects ink,
not material or borders; it is neither another easing nor a blur filter.
Geometry timing is unchanged; the losing motion treatments and prototype hint
have been removed. The earlier Euclidean per-face border correction fixed
diagonal expansion but not half-width silhouettes; centered strokes replace it.

`render/grid/palette.rs` contains five native line-color audition choices. C and
Shift+C select a preview-only palette without modifying Playback or Scene Plans.
The worker's `FrameCache` keys pixels by slide, visual sample, theme, and palette; frame
results carry that palette so a stale in-flight render cannot overwrite a newer
choice. Paused/held frames can change color without advancing their clock. The
renderer override changes cell borders only; exports keep the recipe's default color.
Orange remains the Original theme's default after auditioning the alternatives.
Other themes start with their accent; C auditions override colors for the current
player lifetime, and T restores the newly selected theme's default line treatment.

`crates/psychopomp-render/src/render.rs` is the concrete `wgpu` and `cosmic-text` Adapter. `HeadlessRenderer::render_shapes` renders transparent editor-local geometry into tightly packed RGBA pixels. Cached stable-line sprites, pointers, and annotations are added to that flat editor surface before the shared card compositor presents it.

The Adapter also resolves semantic token targets from `cosmic-text` glyph cluster hitboxes. Token highlights and the pointer consume those measured bounds; choreography does not estimate monospace character widths or hardcode target coordinates.

Code-target measurement shapes each inline partition once for both its advance
and selected cluster bounds, without rasterizing discarded pixels. The separate
`render/text/raster.rs` leaf owns exact glyph rasterization shared with the native
browser bake. `render/text.rs` owns the typed plain-text cache; root, Task, terminal,
and deployment callers retain their different width, line-height, crop, and color
policies. Debug, inline-code, SVG, and bubble resources keep their own lifetimes.

Editor panel translation, three-axis rotation, and scale are sampled properties. The editor shader remains flat and transparent; `render/ui/card.rs` is the sole perspective implementation for both editor and recorded-video surfaces. Depth-weighted Gaussian sampling softens the near edge during the opening pose, while increased entrance shutter sampling keeps fast perspective motion continuous.

Prepared Scene Plans derive an exact visual key from sampled motion position and velocity, prior pointer-motion state, discrete State Track values, terminal source-frame identity, and recipe-owned internal tracks. Shutter samples with the same key render once and contribute their multiplicity to linear-light accumulation. Concrete renderer-owned scenes retain unique time keys because their private recipes may depend directly on time. This optimization preserves arbitrary-time semantics and cannot collapse active motion merely because neighboring encoded frames happen to look similar.

SVG assets are parsed and rasterized once into reusable cached sprites, then transformed and composited for each temporal sample. The pointer uses the exact filled Phosphor `HandPointingIcon` path selected by default in `effect-institute`; the same path supports future SVG actors without adding asset-specific shader geometry.

SVG sprites are rasterized at four times their display resolution and coverage-sampled during rotation. The compositor also supports animated scale, opacity, and blur, which the pointer uses for its Effect Institute-style entrance.

Pointer translation uses mildly underdamped scalar property tracks; rotation is derived from sampled velocity and acceleration so the hand leans against acceleration and follows through while decelerating. The complete transform remains deterministic and participates in temporal accumulation.

Stable code lines can be split into cached stable and variable sprites. An inline reveal opens or closes a variable span's layout width while opacity and blur resolve; every following stable or variable span derives its position from the sampled widths before it. Multiple non-overlapping reveals can exchange Effect and function alternatives horizontally while `const getTime`, ` = `, `getTime`, and `)` retain identity. This is the concrete maximum-stability seam demonstrated by Effect Institute ports and remains narrower than a general recursive slot AST.

Editor text and bright source regions share fractional glyph sampling. Premultiplied bilinear sampling preserves subpixel translation; source-range coverage clips fractional reveal columns once, without double-attenuating raster borders. Weighted blur taps move continuously rather than rounding their offsets. Intersecting line pixels clip against the viewport instead of dropping a complete line at its boundary. Planned text actors use this path too. This intentionally changes preview and export pixels; final-window smooth/pixelated filtering remains separate.

Planned text actors may declare a stationary canvas-space `verticalMask`. The compositor integrates its linear top/bottom fades over each pixel row and applies that coverage to sampled text alpha before blending. Text moves through the aperture; the mask does not follow its center or darken already-composited pixels. The same optional path serves native preview and shutter-sampled export, while unmasked actors retain their original pixels. The rolling showcase captions demonstrate this narrow recipe property without a public clipping tree or new Scene Plan version.

The lesson port extends the compositor to multiple non-overlapping reveals on one stable line. Focus ranges and token highlights carry independent vertical geometry, so cursor-only cues do not accidentally move or resize focus. Semantic annotations currently include a CPU-composited error squiggle and short celebration effects; both participate in temporal sampling.

`crates/psychopomp-render/src/render/effects/` owns the concrete pixel implementations for celebration annotations. Prismatic bloom and focus pulse share one small frame interface but keep their particle and halo recipes in separate files. `render.rs` only translates resolved target geometry into canvas coordinates and dispatches the selected closed recipe; adding another demonstrated built-in effect does not enlarge the renderer adapter or require a plugin interface.

`crates/psychopomp-render/src/render/task.rs` owns the concrete Effect Task visual recipe demonstrated by the `visual-effects` lesson: compressed running nodes, energy sweeps, state flashes, pulses, icons, error bubbles, and labels. The DSL and composition retain stable Task identity and semantic state changes; compiled Task frames also carry the previous state and its completed duration so the renderer can overlap outgoing content and preserve running phase without mutable frame history. This renderer alone owns the current 128-pixel presentation. The same recipe can render a complete Task scene or composite Task actors over existing editor pixels, as required by `effect-is-a-description`. All moving pixel layers share one fractional transform, rounded signed-distance edge, and analytic coverage so the body, sweep, border, pulse, and glow remain one coherent material. Result content keeps its natural transform but is analytically masked by the current sampled rounded body after text blur, preventing spring intermediates from leaking outside the Task without scaling content to fit. A short container-level entrance blur applies to the assembled node while ordinary shutter sampling supplies motion blur from actual movement.

`crates/psychopomp-render/src/render/terminal.rs` owns the concrete OpenCode terminal recipe: source-frame selection, a short split-screen command-file editor, and the missile payoff. The source recording supplies authentic TUI pixels; Psychopomp supplies whole-card camera movement, the external file-write explanation, impact defocus and shake, and foreground effects. Rounded material, border, shadow, perspective, and depth blur are not terminal concepts; the recipe delegates them to the shared card compositor. This is intentionally not a terminal emulator, ANSI parser, generic video layer, or particle plugin.

`crates/psychopomp-render/src/render/deployment_queue.rs` is the first state-driven simulated UI Surface. One typed `deployment-queue` actor owns an ordered catalog and timestamped UI Snapshots rather than one actor per row. During preparation, `plan_runtime/deployment_queue.rs` validates the opaque recipe, computes card-local row targets through the private flow layout, and compiles stable row position, presence, progress, phase-feedback, and attention tracks. `plan_runtime/keyed_layout.rs` is a private generic Implementation behind that recipe; interrupted reorders preserve velocity through the ordinary Timeline compiler, exiting items retain their previous phase until presence settles, and equal-time zero-duration snapshots do not create phantom identity. The recipe contributes its internal sampled values to shutter deduplication and paints a fresh fixed-canvas dashboard for every Temporal Sample without entities, events, retained widgets, or wall-clock animation.

`scenes/opencode-session-tool` combines planned video, layered SFX, continuous card motion, discrete recording state, and named cues through the process seam. Its immutable source recording aligns a real Vim session on the left with one already-running OpenCode v2 client on the right; the concrete terminal recipe preserves those source pixels and their aspect ratio while ordinary `text` actors carry explanatory overlays. The 20.5-second rapid-fire artifact demonstrates live command, agent, project-skill, reference, model, permission, ambient-instruction, and local-plugin generation changes without restarting the OpenCode service, client, or session.

`scenes/deployment-queue` is the first state-driven simulated UI Scene Program. Its typed recipe data owns product copy and a stable service catalog; seven authored state changes drive deployment phases, progress, insertion, failure focus, retry, and final health without authored row coordinates or per-frame calculations. The concrete proof keeps service order stable because focus and color communicate failure priority without a gratuitous full-width row crossing. A phase replacement retains the immediately preceding complete UI Snapshot, keeps row geometry continuous, interpolates color and chip width, sequences old and new text, and transitions aggregate health from the same snapshot pair. Aggregate progress is monotonic, and a stagger is bounded by the next snapshot so delayed presentation cannot outlive newer semantic state.

`scenes/quark-before-after` is a compact narrated tutorial contrasting Solid Store reconciliation with explicit keyed identity. It uses the existing editor, text, and Script Clip recipes rather than introducing a reactive-system visualization or another renderer abstraction. The scene also demonstrates the planned editor's shared perspective-card entrance and multiple channel-driven Inline Reveals: one new Stable Line enters before opposing variable parts exchange inside otherwise stable lines, then the update call changes on its own narration cue.

`crates/psychopomp-render/src/render/ui.rs` is a private, GPUI-inspired immediate-mode vocabulary for renderer-owned pixel interfaces. Immutable `Bounds` values split and inset child regions; `VerticalFlow` places the terminal's source lines. Deployment row centers come directly from their index and fixed row height/gap, not a general layout engine. `render/ui/card.rs` adds nested rounded clips, fills, strokes, packed or strided RGBA sources, fit modes, card-local overlays, and projected cards with one material, border, shadow, surface blur, and depth-dependent near-edge blur. Draw order is z-order and closures provide local composition without retaining public nodes. Flattened editor pixels, decoded terminal-video pixels, and the composed deployment dashboard all enter through this Module. A direct borrowed-source operation preserves the same presentation semantics without first rasterizing a second full-card intermediate; the closure path remains available when a card needs multiple composed layers. The Module intentionally stops before an element tree, flexbox engine, event model, retained widgets, renderer trait, or public UI framework.

The Adapter keeps these details private:

- headless Metal adapter and device creation
- WGSL pipeline and uniforms
- bundled CommitMono (`render/fonts.rs`: compiled-in faces replace any installed
  CommitMono; system fonts are glyph fallback only) and raster-sprite caching by
  stable line ID
- texture-to-buffer row alignment
- asynchronous mapping and GPU polling
- the flat dark editor treatment

There is no renderer trait. One Adapter is a hypothetical seam; a second backend would make it real.

### Explainer overlays

`sequence` and `caption` are CPU overlays with strict payloads (`deny_unknown_fields`)
and strict channel names: `plan_runtime/sequence.rs` and `plan_runtime/caption.rs`
reject any property on their actor that does not name a real participant, row, or
recipe channel, because a typo would otherwise do nothing. Geometry and validation
live in the lightweight crate (`sequence.rs`, `caption.rs`), so authoring helpers
(`SequenceActor`, `CaptionActor`) and tests need no GPU. Pixels come from
`render/sequence.rs` and `render/caption.rs` through the shared primitives: cached
CommitMono plain-text sprites, `UiCanvas` fills, and the analytic polyline stroke.
Sequences draw with the other diagram surfaces (after Venn and Value Tokens);
captions draw above rich text and below plain text. Semantic colors resolve through
`Theme::tone`, the one place status colors are fixed (Neutral overrides them).

A `rolling-number` overlay (drawn just after captions) carries its own timed
values, like editor or grid snapshots, rather than a State Channel. The
lightweight `rolling.rs` tokenizes each value into stable digit places,
separators, and literals, then folds the changes in order into closed-form
settling tracks (`math::dynamics::settle`) per column: x, wheel position,
opacity, and rise. Each change samples the earlier tracks at its time, so a
redirect keeps position and velocity, and any time samples without history.
The renderer supplies only glyph advances to that compilation, then paints each
wheel's two straddling faces through a stationary `VerticalMask` window with a
speed-driven vertical smear (`TextFilter::Smear`). Because its motion lives
outside Continuous Channels, a settling number marks its samples distinct
(`ambient_time`), including over a Stage, so shutter samples are not merged. Its
schedule follows the authored clock, so plans using it are export-only.

`plot` and `lanes` are chart overlays (drawn just after sequences) that share an
axis vocabulary. The lightweight `axis.rs` owns `AxisPlan` (range, ticks, label,
unit) with tick choice (`every`, `nice`) and tabular tick labels; `plot.rs` and
`lanes.rs` own payloads, validation, strict channel matching (`accepts`, which
matches `series.<id>.…` and `lane.<id>.…` by prefix and suffix so ids may contain
dots), interpolation (`PlotSeriesPlan::y_at`, `slope_at`), and the `PlotActor`
and `LanesActor` handles. A plot's curves are points the Scene Program sampled,
optionally with exact slopes; the renderer never evaluates a function.
`LanesPlan::from_scene_plan` compiles selected channels with the ordinary
`compile_channels` to sample their sparklines, so a Lanes view shows the tracks
the renderer would play. Pixels come from `render/plot.rs` and `render/lanes.rs`
over `render/chart.rs`, which holds the shared ink: snapped CommitMono labels,
`chart_axis` (ticks disclosed as the line reaches them, fading beside a playhead
readout), dashes along arc length, dots, diamonds, and readout tabs. Both draw
only from channels, so they are native-presentable.

### Stage

`stage` is an exclusive root recipe. The lightweight crate (`stage.rs`) owns the
element model, strict channel names, the perspective `Camera`, element outlines, and
the deterministic orb geometry (Fibonacci points, shatter trajectories), so authoring
helpers (`StageActor`: `to`, `ease`, `clock`, `hit`, `send`, `type_in`) and tests need no GPU.
`render/stage.rs` is small pieces: `Scene` samples the camera, every element's
placement, and every beam's path once per sample; `Painter` has one method per
element kind; `StageFrame` owns primitive helpers and depth-sorted layers. A beam is
a `math::shapes::connect` connector between the two outlines on screen: it leaves the
middle of the card side that faces the other end, perpendicular to it, and enters an
orb radially beneath its shell, with a socket where it plugs into a card. A dark
orb body occludes submerged endpoints and packet landing rings; packet labels
fade before entering the shell. Packets sort behind the connected bodies too.
A packet is one clock (`age`, `flight`); `stage::packet` derives its phases (gather,
minimum-jerk quintic flight by arc length, landing ring, and a trail whose points cool with
the time since the packet crossed them, found by inverting the ease), so rewinding
the clock un-cools the trail. Lights are collected per sample from packets and
drawing beams: a reflection (edges only, the diagrams' radial falloff) and pools
(ember, flood, surge) that also enter the glass. Each card takes its strongest
reflection and strongest pool as two shader lights; the orb's shell points sum
them. Polyline points carry a heat that scales their light. A beam
sorts behind both of its ends, so it never crosses the cards it connects. The
renderer emits depth-sorted signed-distance primitives (rounded rect, circle, arc, polyline with drawn length,
dash, flow, and fade, atlas text, backdrop gradient) into one storage buffer;
`stage.wgsl` draws them as instanced quads into an `Rgba16Float` target with
premultiplied blending, where glow adds light at zero alpha. `stage_post.wgsl`
thresholds and blooms that light through a five-level 13-tap downsample and tent
upsample, then composites with highlight rolloff (identity below 0.8, so authored
UI colors stay exact), chromatic aberration, vignette, and grain locked to the
output frame. A Stage exposes a frame on the GPU: each of its 24 temporal samples
draws into the HDR target and adds, weighted, into an HDR exposure target; bloom,
rolloff, and grain then develop that exposure once, with the post settings of the
central sample. Light therefore integrates before the response curve (a bright
ember keeps its streak's energy), and no sample is read back. Plan overlays drawn
over a Stage (headers, chips, captions) are composited once, or averaged on the
CPU only across samples where their own state differs. Stage text is rasterized
once at twice its size into an R8 atlas and drawn with a soft background-colored
backing for legibility over light. A stage root marks every temporal sample as
distinct (`ambient_time`), because spin, flow, and grain always move.

Every root renders a frame from one exposure, `exposure::exposure`: stratified
times across a 180-degree shutter whose weights ease off over the outer quarter
at each end, so streaks fade rather than ending on a hard copy. Samples with
equal visual keys merge their weights. Roots without their own exposure average
sRGB samples in linear light on the CPU (`exposure::accumulate`).

The Stage separates material response from transforms: a pulse lights the orb
without moving its shell or ports, and a card flash lifts ink and rim while its
substrate stays dark. An overhead key shades panel fills and borders; moving
reflections stay local. `settle_in` uses small, damped scale/position springs with
a separate delayed content spring; `ease(.., Ease::Smootherstep)` suits deliberate
camera compositions, and `clock` starts an effect rig's elapsed-seconds channel. Packet travel uses the same acceleration-continuous quintic,
with its inverse in shared math providing trail crossing times.

Orb `rotation` is an angular offset, independent of ambient `spin`; `blur` is a
separate defocus pose. `burst` is an opt-in age in seconds (-1 means intact): a
120 ms collapse, combustion, smoke, and embers over 5.2 seconds. The private
`effects/combustion.wgsl` and `effects/noise.wgsl` are concatenated with the primitive shader and raymarch a
domain-warped procedural density with emission and Beer-Lambert absorption.
It is an analytic appearance, not a fluid simulation. Embers use shared
`effects::combustion::Burst` over `math::dynamics::ballistic` (constant gravity and linear drag), while the composite
refracts the scene with an inward pinch and expanding pressure wave. The first
active orb in element order drives this screen-space wave; volumes and embers
render for every bursting orb. Reversing the age reconstructs the effect without
simulation history; ambient spin remains on the scene clock.
The intact shell keeps its material and occlusion through compression, then
hands presence to the hot particles over a 55 ms ignition envelope. Combustion
casts an age-driven local rim reflection through the existing light path; it
does not wash card fills. The procedural density has compact support, reaching
zero before the ray interval and screen-space rejection bounds.
The binding-free `effects/pressure.wgsl` returns a displacement field for the
composite. These concrete Modules form the initial [effects library](EFFECTS.md);
effect physics and shader optics can be reused without Stage identities.
`effects/rewind.wgsl` adds the blog's VHS tape interference (tear, snow on ink,
scanlines) to the same composite, controlled by `post.rewind`; it needs no
previous-frame textures or feedback. Card deletion (glitch bands, hairline cut)
clips copies of the card's primitives by their bounding quads, since every
primitive rasterizes only inside its box. `Theme::Neutral` is the blog's "clear
neutral" palette: quiet frames and wires, ivory signals, and desaturated
semantic inks; it is the one theme whose status tones differ.
Incoming packets also sample `effects::surface` from the first visible-shell
contact, found by `Circle::entry_fraction` and the inverse packet travel curve.
The local dimple, particle emission, and hemisphere-masked spherical trace travel
outward from that contact; they do not scale the receiver or shift its ports.

Editor diff backgrounds union their weighted vertical intervals before pixel
coverage (`render/line_marks.rs`). Adjacent fractional rows therefore share a
single tint instead of double-blending an antialiased seam. Gutter signs remain
attached to the sampled line positions.

### Eased events

`TrackEventPlan::Ease` lowers to `Animation::Ease` and a `SegmentKind::Ease`
segment: position from `math::easing::Ease::sample`, velocity from its derivative
`slope`, settled exactly at the end. It replaces stepped `set` approximations of
timed curves, which stutter at 60 fps, and hands its velocity to a later spring.

### Math

`psychopomp::math` is the shared vocabulary for motion and geometry, grouped like
pmndrs `math`: scalar `lerp`, `inverse_lerp`, `remap`, `remap_clamp`, and
`smoothstep`; glam's `Vec2`, `Vec3`, and `Quat`; `easing` curves; `curve`
(`CubicBezier`, and `Polyline` with arc-length sampling and slicing); `shapes`
(`Box2`, `Circle`, `Shape` outlines with facing `Port`s, `connect`, and
`fibonacci_sphere`); and `random::hash`. Renderers and Scene Programs compose these
instead of carrying private lerps, easings, or geometry.

Live shaders: when `PSYCHOPOMP_SHADER_DIR` is set, the stage reads `stage.wgsl` and
`stage_post.wgsl` from that directory instead of the compiled-in copies, so each
`plan frame` or sheet reflects shader edits without recompiling Rust.

The editor adds `panel-x` (a translation of the projected card), `panel-opacity`,
and Line Marks. Marks draw in the shared text pass before code, so the native
preview path and the projected export path agree; a non-zero `panel-x` or partial
opacity disables the preview shortcut. `inlineReveal` is optional.

`psychopomp::highlight::typescript` compiles one TypeScript line into styled spans
for editor recipes. It is a line-local approximation for explainers, not a parser.
`psychopomp::editor::diff` builds a Stepped Diff on top of it: an `editor` actor
whose lines keep identity across steps, with room-opening snapshots and removed
lines' `mark.*` channels turning red just before they leave.

## Encoding Is One Concrete Adapter

`plan_runtime/delivery.rs` owns PNG and MP4 delivery separately from `PreparedPlan` preparation and sampling. Video exports keep the authored timeline, full visual quality, shutter samples, and original audio placements.

A Reel (`plan::ReelPlan`) is delivery, not a scene. The lightweight crate owns its
validation and timing: `spans` places segments on one clock and `layers_at` returns
the one or two segments visible at a time with eased mix weights. `plan_runtime/reel.rs`
prepares every segment once on one renderer, samples each visible layer at its local
time, and mixes opaque frames (a dip mixes over the theme's empty background). Each
segment's media placements are shifted onto the reel clock with
`MediaPlacement::shifted` and encoded through `exposure::encode_exposures`, the same
exposure and FFmpeg path as a single plan. A segment shown alone through a frame
renders its own exposure (so a Stage keeps its GPU shutter); mixes and zooms take
16 samples averaged on the CPU. `plan render`, `frame`, `snapshot`, `validate`,
and `inspect` recognize a reel by its `segments` key; `--cue` selects
one segment by scene ID. A zoom transition resolves each layer's
`ReelZoom` transform (geometric scale, focus-to-center travel, rounded corners on
the incoming card) and warps the rendered frames bilinearly before mixing; zoom
progress is part of the sample key so the move keeps its motion blur.

`psychopomp/src/playback.rs` derives numeric step destinations from a renderer-prepared Timeline, after semantic geometry has resolved. Next, Previous, First, and Last append only changed channel targets through the shared Timeline compiler. Each spring therefore inherits position and velocity, including mid-flight reversals; unchanged destinations do not restart motion. Per-channel motion profiles come from the destination's latest authored spring (or its first spring before any event; set-only channels use a 0.4-second zero-bounce default). Replay alone resets to the entry pose. The local clock freezes on pause or once all channels settle, without retiming the authored video. Immutable `Arc<Timeline>` revisions make sampling history-independent even while input creates a newer revision.

`plan_runtime/presentation.rs` owns winit lifecycle, slide/step navigation, full screen, letterboxed resizing, and smooth/pixelated display filtering. `presentation/worker.rs` retains the deck's prepared scenes, fonts, and GPU. At most one render is in flight; requests and results carry slide identity and immutable timeline revisions, so switching slides cannot display a stale result from another scene. Each slide retains its selected step and paused local clock while inactive. Explicit pause stays paused on return; previously running motion resumes. Held/paused scenes sleep unless a planned Task requests ambient clock advancement. Generic State Channels and recorded media remain unsupported by interruptible playback; video support is unchanged.

The GPU-free `presentation/scheduler.rs` owns request eligibility, complete-sample
equality, invalidation, completion freshness, and phase-preserving deadlines.
`RequestStamp` provenance is distinct from the worker's visual cache key and the
uploaded pixel `Arc`. A stale completion releases the one-in-flight slot without
replacing front pixels or clearing a pending repaint.

Native inspection uses `PlaybackSpeed` (1x, 0.5x, 0.25x, 0.1x) to divide elapsed
wall time. A speed change reanchors at the same local time without rebuilding the
Timeline. Frame steps change only the paused sample cursor by 16.667 ms and stop
backward movement at the latest navigation boundary. Both operations increment
the presentation revision, rejecting old in-flight frames. Shift+R uses ordinary
Replay followed by pause so the first frame and delayed word starts can be inspected.

`presentation/debug.rs` captures local/navigation time, speed, phase, and pending
start counts with the render request. Header readouts sample each word's actual
spring progress. `render/debug.rs` paints a bounded optional HUD over a clone of
the worker's clean cached frame; hiding it restores the exact clean cache object.
The HUD has at most eight replace-in-place glyph-cache slots and never enters
Scene Plans, visual sample keys, or file exports. Speed/debug are session-local,
not saved alongside theme preferences; benchmark mode rejects slow/debug options.

Semantic coordinates retain a numeric expanded-layout baseline plus private companion weights compiled by `plan_runtime/attachments.rs`. Sampling applies each weight to the difference between expanded and visible target geometry, including product-rule velocity. Target measurements use the same span partitions and shaping as inline painting; weight tolerances are normalized to the measured coordinate extent. Companion tracks participate in native retargeting, preserving continuity when switching between moving targets and literal coordinates. Line-layout and reveal drivers must use literals: semantic feedback into their own geometry is rejected. These are prepared renderer tracks, not new public Scene Plan fields.

`psychopomp/src/task.rs` describes planned Task states, and `plan_runtime/task.rs` lowers them into continuous geometry, color weights, activity, and independent content/bubble channels. `TaskVisualFrame` reuses the existing Rust Task material, icons, result clipping, error bubbles, jitter, and energy sweep instead of importing a browser runtime. The source Pixi profiles remain distinct: 0.2-second/bounce-0.5 height, 0.35-second/bounce-0.35 width, approximately 0.167-second icon opacity/scale/blur, and 0.25-second/bounce-0.4 result scale with 0.15-second deblur. State content can reverse mid-transition through the shared Timeline. The activity track keeps the local clock and visual sample key advancing; pause, slide departure, and reduced motion freeze it. This is a scoped native adaptation of Effect Institute's Pixi blocks, not a general implementation of browser component state or actual Effect execution.

`render/task/content.rs` contains sampled `ContentPose`, `BubblePose`, and `TaskContentFrame` values. It does not remap a shared state-progress spring through a second easing or delayed visibility window. Preparation compiles `content.<state>.*` and `bubble.<state>.*` channels, including the source's independent bubble rise/fade/deblur. These channels enter native Playback like any other trajectory; no edge-triggered timer or direction branch can reset a reversing pose. The symbol rotates independently while its clip remains aligned with the body. Error text, background, and tail share one cached bubble sprite so its entrance transforms the whole surface. An outgoing bubble is not culled by its faster icon fade. Unchanged channels keep content still. No public Scene Plan fields or generic transition framework are added.

The source's instantaneous running toggle becomes a short 0.06-second continuity ramp, without waiting for content to disappear. Retained symbols use a symmetric 0.7 hidden scale instead of the source's new-icon mount resets and 0.9 exit scale; selective symbol rotation remains an intentional native flourish. Results retain a 0.5 hidden scale. These adaptations preserve arbitrary-step continuity, so the timing tests claim source spring-curve parity, not complete source-renderer pixel equivalence. `tests/fixtures/effect-task-timing.json` comes from the source project's pinned Motion DOM 12.42.2 generator and tests actual compiled channel samples, including overshoot and separate result deblur.

Transformed sprites use denser 5×5 binomial filter taps: the larger content defocus exposed visible displaced copies with the former 3×3 taps, particularly in error-bubble text. Tap positions still vary continuously, and the unblurred identity case retains direct bilinear sampling. This improves a sampled optical effect, not the trajectory or the export shutter model.

`presentation/gpu.rs` owns the final native surface, independently of scene recipes. It uploads changed RGBA frames to one sRGB texture and draws a letterboxed triangle with linear or nearest filtering. FIFO presentation requests one-frame latency. Resizing or switching filters reuses uploaded pixels; lost/outdated surfaces are rebuilt and zero-sized/occluded windows defer presentation. This replaces the measured CPU scaling/softbuffer bottleneck without migrating choreography or code layout to another renderer. `presentation/benchmark.rs` measures one warmup plus nine native interruption rounds; `perf/native-playback.md` records the comparison and its limits.

Native sampling follows the current monitor's reported millihertz refresh rate (60 Hz fallback), with an explicit `--fps` override independent of video timing. Moving, resizing, or refocusing the window rechecks the rate. Neither setting changes FIFO synchronization or the one-in-flight worker bound. `--benchmark` records display rate and acquisition wait; `--benchmark-gpu` additionally waits for each submitted draw to finish and reports scene sampling plus completed upload/draw work, excluding drawable acquisition. That diagnostic wait is never enabled during ordinary playback. Display refresh, rendering headroom, and verified scanout FPS are distinct measurements.

The native preview is a deliberate quality profile, not a second choreography implementation. A neutral editor with no visible pointer or annotation reuses static composed chrome and paints code directly, omitting export's final optical resampling. Dynamic focus/highlight overlays use the same WGSL recipe in a chrome-free pass, avoiding an optical-card rebuild whenever an attached highlight moves. Unsupported transforms/effects fall back to the full renderer. `--full-quality` disables this shortcut; export never enables it. Pixel equality is required across sampling order within a profile, not between profiles. Higher-DPI glyph rasterization, presentation audio, and live source reloading remain future work.

The cheap overlay pass is limited to bounds safely inside the card body; overlays that can overlap title/border pixels or escape its clip use the full path. Two filename-keyed chrome images are retained, and each slide's initial resources are warmed before the native window opens. Frame deadlines retain their phase across late wakes rather than drifting relative to the previous request. Missed slots are skipped, not queued.

`crates/psychopomp-render/src/encode.rs` owns the FFmpeg process, raw-frame protocol, audio placement filters, argument construction, and exit validation. The Interface accepts tightly packed RGBA frames plus compiled audio media placements. FFmpeg trims immutable source ranges, applies each clip's non-destructive gain, shifts clips onto the composition clock, mixes overlapping layers through a peak limiter, and encodes AAC beside H.264.

FFmpeg remains a subprocess because it avoids unsafe bindings and codec linkage while preserving access to the installed encoder set. A second encoder is not currently justified.

## Scene Modules Own Choreography

`crates/psychopomp-render/src/main.rs` parses the command, selects an output, and dispatches to one concrete Module under `crates/psychopomp-render/src/scenes/`. Each scene Module keeps its assets, documents, snapshots, semantic targets, choreography, and sample rendering local behind one `render(output)` interface. `crates/psychopomp-render/src/scenes/mod.rs` contains only mechanics shared by these scenes: the `encode_scene` entry, semantic target measurement, the editor frame, pointer sampling, and styled-span construction. Delivery dimensions, shutter exposure, and accumulation live in `crates/psychopomp-render/src/exposure.rs`, shared with Scene Plans and reels.

This is a locality seam, not a scene framework: there is no scene trait, registry, or generic lifecycle. Shared authoring operations should move into the DSL only when repeated usage reveals a deeper interface.

The hero is the first complete exception to direct scene-module execution. `scenes/hero` is a lightweight Rust Scene Program containing its stable editor document, logical semantic ranges, actors, channels, and exact choreography. It emits `scenes/hero/hero.plan.json`; a byte-equality test keeps that generated canonical plan synchronized with Rust source. The default renderer embeds the plan for command compatibility, resolves its editor targets through `crates/psychopomp-render/src/plan_runtime/editor.rs`, and renders through the shared persistent-plan path. The former direct `scenes/hero.rs` implementation was removed after the full 300-frame H.264 artifact matched byte-for-byte.

`crates/psychopomp-render/src/scenes/effect_institute.rs` is the private exception for a demonstrated external corpus: 30 published sections across the first two Effect Institute chapters. It reads pinned, checked-in compiled lesson artifacts rather than accepting user-authored JSON. Stable template line, part, and version IDs lower into ordinary Psychopomp line/opacity/part property tracks; published step times retarget those tracks with the same line and inline motion contracts used by hand-authored ports. Component snapshots lower into the existing concrete Task recipe or one closed section-local presentation. The adapter does not add a public JSON authoring boundary or generic scene graph.

Published cursors, token highlights, focus regions, and long-section camera offsets compile through the same explicit-time Timeline segment compiler used by Scene Plans rather than a second analytic track implementation. Their retargets preserve velocity at rapid cues, while hidden component actors stay mounted so mode changes use Task entry/exit motion instead of changing row identity. Imported component snapshots and authored Tasks both use the generic `StateTrack<TaskState>` implementation for previous value, state age, and visibility intervals. The imported editor clips and softly fades camera-edge content to its code viewport.

A stitched chapter is one encoder job over exact section narration placements, but each section keeps a zero-based local clock and independent actor namespace. One exact `TimeRange` interval schedule derives both media composition and visual selection; lesson IDs compile into global named cues. Unrelated sections do not interpolate actor identity or momentum across boundaries.

## The Rust DSL Produces Pure Values

`crates/psychopomp/src/dsl.rs` is the existing typed authoring boundary exported through `crates/psychopomp/src/lib.rs`. Authors compose property `Motion`, semantic `Annotation` values, Task state and pose changes, and media in a `Composition`. `crates/psychopomp/src/author.rs` adds stable handles for Scene Programs that emit versioned plans. Both frontends lower continuous changes into the same Timeline compiler and discrete values into generic State Tracks.

`CodeEdit` is the first authoring operation extracted from repeated scene usage. It owns the layout/content tracks for one coordinated structural edit, supplies their initial values, returns enter/exit Motion, and samples a `CodeTransition` against a compiled Scene. It does not absorb `CodeTransition` or hide intentionally staggered tracks such as the hero's separate layout and content cues. `Cue::at` similarly centralizes exact cue-start placement for any composable leaf without teaching cues about Motion, Tasks, annotations, or media roles.

Scalar targets may remain semantic while authoring. The in-process DSL uses `TextTarget` and `Scalar`; Scene Plans use stable `SemanticTargetPlan` declarations and `ScalarPlan` target references. Both request target edges, widths, centers, line positions, or attached offsets and resolve geometry once before compiling the same numeric Timeline.

The hero and lesson ports are concrete clients of these APIs. Scene Programs may serialize generated Scene Plans as JSON for the process protocol, but JSON and TypeScript are not source authoring languages.

## Current Stack Decisions

- [`wgpu 30`](https://github.com/gfx-rs/wgpu) is the headless GPU substrate. Rendering targets an offscreen texture without a window or surface.
- [`cosmic-text`](https://github.com/pop-os/cosmic-text) shapes and rasterizes CommitMono lines into stable cached sprites.
- [`glyphon`](https://github.com/grovesNL/glyphon) was evaluated and removed: repeatedly preparing a dynamic GPU glyph atlas across temporal samples can invalidate glyph coordinates during atlas growth. Cached line sprites fit Psychopomp's stable-identity model better.
- WGSL remains the shader language because it is native to wgpu and translated by Naga.
- An FFmpeg subprocess handles H.264 encoding. [`ffmpeg-next`](https://github.com/zmwangx/rust-ffmpeg) is maintenance-only and adds an unnecessary FFI seam.
- [`Vello`](https://github.com/linebender/vello) remains deferred because its API and wgpu compatibility are still moving. `lyon` is the likely addition if authored vector paths become necessary.
- The current RGBA8 render target is sufficient for the visual prototype. Temporal samples are decoded to linear light before CPU accumulation and converted back to sRGB once per output frame, avoiding dark gamma-space motion trails. A production compositor should render and accumulate directly in linear `Rgba16Float`, then tone-map into the delivery color space.

## Explicit Non-Abstractions

The prototype does not have a generic scene graph, renderer trait, plugin interface, render graph, dynamically loaded Rust library, or recursive slot AST. The two-crate workspace exists only to keep lightweight Scene Programs independent from the heavyweight persistent renderer. Further package seams require another demonstrated compilation or deployment need.
