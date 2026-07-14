# Prototype Architecture

Kinograph is organized around a small set of Modules whose Interfaces correspond to demonstrated change seams: code identity, media composition, motion trajectories, scalar tracks, rendering, and encoding.

`src/timeline.rs` compiles pure animation values into deterministic scalar property tracks. Its renderer-neutral Interface drives the hero scene's panel, code-layout, code-content, focus, highlight, and pointer properties while preserving velocity across spring retargeting.

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

`src/code.rs` owns stable line IDs, code documents, snapshots, validation, and line placement. Its Interface is `CodeTransition::compile` followed by `CodeTransition::sample`.

The Module does not know about glyphs, GPUs, colors, FFmpeg, or absolute screen coordinates. This gives the identity and layout rules locality and makes snapshot behavior testable without external systems.

The current model intentionally stops at line identity. Inline part identity, named slots, and content versions should enter this Module only when the next authored scene requires stable inline replacement.

## Motion Carries Position and Velocity

`src/motion.rs` owns the analytic damped-spring equation. Its Interface accepts an initial motion state, target, and arbitrary elapsed time.

This Interface provides leverage beyond easing: deterministic out-of-order sampling and future momentum-preserving interruptions use the same Implementation. Timeline compilation remains deferred until a scene contains an actual interrupted transition.

## Composition Owns Cross-Media Time

`src/composition.rs` keeps immutable source assets separate from their uses in an edit. A `Clip` selects an exact source range; compiling a `Composition` places that range on the output timeline without modifying the asset.

Composition supplies sequence, parallel, delay, hold, and named cue ranges across both visual `Motion` and timed media. It distinguishes transcript-bearing script clips from accompanying layer clips such as sound effects, music, and B-roll. This mirrors the real behavioral difference exposed by transcript-led editors without introducing a graphical editor or media decoder.

Media time is stored as integer nanoseconds. Conversion to floating-point seconds happens only when visual motion is lowered into scalar trajectories, so repeated source-range edits retain exact boundaries.

Still images remain stable scene actors rather than pretending to be time-based clips. A compiled scene retains each image asset and its transform property IDs; actual image composition remains renderer work for the first image-backed scene.

## Rendering Is One Concrete Adapter

`src/render.rs` is the concrete `wgpu` and `cosmic-text` Adapter. `HeadlessRenderer::render_shapes` renders scene geometry into tightly packed RGBA pixels. `HeadlessRenderer::composite_text` places cached stable-line sprites into every temporal sample before accumulation.

The Adapter also resolves semantic token targets from `cosmic-text` glyph cluster hitboxes. Token highlights and the pointer consume those measured bounds; choreography does not estimate monospace character widths or hardcode target coordinates.

SVG assets are parsed and rasterized once into reusable cached sprites, then transformed and composited for each temporal sample. The pointer uses the exact filled Phosphor `HandPointingIcon` path selected by default in `effect-institute`; the same path supports future SVG actors without adding asset-specific shader geometry.

SVG sprites are rasterized at four times their display resolution and coverage-sampled during rotation. The compositor also supports animated scale, opacity, and blur, which the pointer uses for its Effect Institute-style entrance.

Pointer translation uses mildly underdamped scalar property tracks; rotation is derived from sampled velocity and acceleration so the hand leans against acceleration and follows through while decelerating. The complete transform remains deterministic and participates in temporal accumulation.

Stable code lines can be split into cached prefix, reveal, and suffix sprites. An inline reveal opens the inserted span's layout width while opacity rises and blur resolves; the suffix position is derived from the sampled width. This is the first concrete inline-identity seam and remains narrower than a general recursive slot AST.

The Adapter keeps these details private:

- headless Metal adapter and device creation
- WGSL pipeline and uniforms
- CommitMono loading and raster-sprite caching by stable line ID
- texture-to-buffer row alignment
- asynchronous mapping and GPU polling
- the flat dark editor treatment

There is no renderer trait. One Adapter is a hypothetical seam; a second backend would make it real.

## Encoding Is One Concrete Adapter

`src/encode.rs` owns the FFmpeg process, raw-frame protocol, argument construction, and exit validation. The Interface accepts tightly packed RGBA frames of one declared size.

FFmpeg remains a subprocess because it avoids unsafe bindings and codec linkage while preserving access to the installed encoder set. A second encoder is not currently justified.

## Main Owns Choreography

`src/main.rs` is intentionally visible orchestration. It declares the hero code document and snapshots, samples named motion profiles, evaluates eight shutter samples, averages them, and sends one frame to the encoder.

Choreography should move behind a scene compiler only after a second scene reveals repeated authoring operations. Moving it now would create a shallow pass-through Module.

## The Rust DSL Produces Pure Values

`src/dsl.rs` is the public authoring boundary exported through `src/lib.rs`. Authors compose property `Motion`, then place it alongside media in a `Composition`. Typed actors such as `Pointer` return ordinary motion values rather than executing animation.

Scalar targets may remain semantic while authoring. `TextTarget` identifies meaningful code content; `Scalar` expressions request its left edge, width, center, or attached offset. `Scene::compile` resolves measured target geometry once and lowers the complete tree into a `CompiledScene` containing the renderer-independent property `Timeline`, scheduled media placements, named cue ranges, and total duration.

The current hero is the first client of this API. There is no JSON or TypeScript authoring boundary.

## Current Stack Decisions

- [`wgpu 30`](https://github.com/gfx-rs/wgpu) is the headless GPU substrate. Rendering targets an offscreen texture without a window or surface.
- [`cosmic-text`](https://github.com/pop-os/cosmic-text) shapes and rasterizes CommitMono lines into stable cached sprites.
- [`glyphon`](https://github.com/grovesNL/glyphon) was evaluated and removed: repeatedly preparing a dynamic GPU glyph atlas across temporal samples can invalidate glyph coordinates during atlas growth. Cached line sprites fit Kinograph's stable-identity model better.
- WGSL remains the shader language because it is native to wgpu and translated by Naga.
- An FFmpeg subprocess handles H.264 encoding. [`ffmpeg-next`](https://github.com/zmwangx/rust-ffmpeg) is maintenance-only and adds an unnecessary FFI seam.
- [`Vello`](https://github.com/linebender/vello) remains deferred because its API and wgpu compatibility are still moving. `lyon` is the likely addition if authored vector paths become necessary.
- The current RGBA8 target is sufficient for the visual prototype. A production compositor should render and accumulate in linear `Rgba16Float`, then tone-map into the delivery color space.

## Explicit Non-Abstractions

The prototype does not have a generic scene graph, renderer trait, plugin interface, render graph, multiple crates, or recursive slot AST. Each would increase Interface cost before providing leverage.
