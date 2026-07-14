# Prototype Architecture

Kinograph currently has four deep Modules. Each Interface hides a concentrated Implementation and corresponds to a real change seam in the prototype.

`src/timeline.rs` is an emerging fifth Module. It compiles pure animation values into deterministic scalar property tracks. Its current Interface is renderer-neutral and drives the hero scene's panel, code-layout, code-content, and focus properties while proving composition, conflict detection, and velocity-preserving spring retargeting before an authoring frontend is chosen.

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
