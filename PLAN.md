# Kinograph Prototype Plan

Kinograph is a code-first motion graphics system for making animated technical videos. It combines Manim's semantic scene construction, Remotion's deterministic programmatic timeline, and the state-driven code animation DSL in `effect-institute`.

`PRIOR_ART.md` records the specific animation systems and API ideas that should be pressure-tested before committing to an authoring model. In addition to Manim, Remotion, and `effect-institute`, the current research set includes Motion Canvas, Motion/React Motion, and Theatre.js.

The prototype should prove one claim: a small declarative program can produce code animation with noticeably better weight, continuity, camera movement, and motion blur than a typical Remotion composition.

Kinograph is not initially a Remotion replacement. It is an opinionated compiler and renderer for a narrow class of kinetic technical videos.

## The First Artifact Is One Excellent Scene

The first artifact will be a 5-10 second, 1920x1080, 60 fps video containing:

1. A code editor panel entering the frame.
2. Stable code transforming through two or three authored states.
3. Line-level and token-level focus.
4. A camera reframing the focused code.
5. An annotation or callout entering in coordination with the code.
6. Continuous spring motion with no velocity discontinuities.
7. Shutter-based motion blur during fast movement.
8. An H.264 MP4 produced without a browser.

This scene is the benchmark. Architecture that does not improve the scene or authoring loop waits until later.

## The Author Describes Meaningful States

Authors should describe what changes, what remains stable, and when an event occurs. They should not calculate frame numbers or interpolate properties themselves.

An early TypeScript sketch captured the desired semantic authoring style:

```ts
export default video("effect-service", {
  canvas: hd({ fps: 60 }),
  theme: ember,

  scene: scene(function* ($) {
    const editor = $.code("service", stack(
      line("tag", "class UserService extends Context.Tag(\"UserService\")<"),
      line("self", "  UserService,"),
      line("shape", [
        "  { readonly ",
        slot("method", {
          initial: "find: (id: string) => Promise<User>",
          effect: "find: (id: string) => Effect<User, NotFound>",
        }),
        " }",
      ]),
      line("close", ">() {}"),
    ))

    $.stage(editor.center().size(1480, 820))
    $.camera.frame(editor, { padding: 140 })

    yield* $.enter(editor, { from: down(72), scale: 0.96 })
      .spring("product")

    yield* $.wait(180)

    yield* $.parallel(
      $.change(editor, { method: "effect" }),
      $.focus(editor, { lines: ["shape"], tokens: ["Effect<User, NotFound>"] }),
      $.camera.frame(editor.lines("shape"), { padding: 260 }).spring("camera"),
    )

    yield* $.callout("Errors become visible")
      .attach(editor.token("NotFound"), "right")
      .enter({ from: right(28) })

    yield* $.hold(900)
  }),
})
```

This syntax remains prior design evidence rather than the selected authoring language. The prototype discovered its public API by producing the hero scene in Rust.

## Rust Owns Authoring And Plan Compilation

The renderer, motion engine, and public authoring DSL are Rust. A lightweight Rust Scene Program constructs typed values and emits a versioned Scene Plan. The plan is compiled output, not a user-authored JSON language; ordinary source retains Rust imports, calculations, loops, helpers, and type checking.

The Rust API carries forward the important domain lessons from `effect-institute`:

- `line` gives stable structural identity.
- `slot` describes a controlled state change.
- `stack` and inline arrays compose code structurally.
- snapshots make each step inspectable.
- `on(word)` synchronizes changes to narration.
- focus and annotations target semantic content.
- stability analysis catches unnecessarily replaced text.

```text
Rust Scene Program
       |
Scene Plan: actors + continuous channels + state channels + exact media and cues
       |
persistent kinograph-render process
       |
scene sampled at global time t
       |
wgpu pixels + FFmpeg media assembly
```

Published Effect Institute chapters may enter through a private artifact adapter when the compiled corpus retains stable identity and exact timing. This is a delivery path for pinned first-party lessons, not a public JSON authoring boundary; ordinary authored scenes continue to use Rust values directly.

## The Scene Plan Preserves Identity

The Scene Plan represents timed intent rather than flattened drawing commands. Its first version has five concepts.

### Actors

An actor is a stable visual object such as a code block, panel, label, callout, or camera. Every actor has an ID, a renderer recipe, and recipe-owned data. The core does not interpret that data.

### Code Structure

A code adapter contains stable lines and flat stable inline parts. Slot changes replace only parts authored as variable. This carries the `effect-institute` stability principle into rendering without teaching the general plan core about editors.

Identity should initially be authored rather than inferred:

```text
code block -> line -> span -> glyph
```

Automatic token matching between arbitrary source files is a later feature. Authored slots provide better results with much less complexity.

### Channels

A continuous channel is one scalar actor property with ordered set and spring events. A state channel is one discrete actor property with ordered values. Both use explicit media time and preserve equal-time source order.

### Cues

A cue names a range on the timeline. Cues can wrap an authored composition or come from narration word and phrase timing. Their start and end provide semantic synchronization points; explicit time remains an escape hatch.

### Transitions

A continuous transition retargets one property with a motion profile. The renderer compiles these events into analytic trajectories carrying position and velocity. A state transition retains previous value, previous duration, transition time, and age without frame history.

## Motion Is a Core Domain, Not an Easing Function

Kinograph should not model movement as `lerp(start, end, easing(progress))`. That representation loses velocity whenever a movement is interrupted or redirected.

Each animated scalar should compile to a continuous trajectory carrying at least:

```rust
struct MotionState {
    position: f32,
    velocity: f32,
}
```

A spring transition starts from both the current position and current velocity. An interrupted camera move therefore continues with existing momentum instead of visibly restarting.

The first motion implementation should use an analytic damped spring. An analytic solution is deterministic at arbitrary timestamps and avoids accumulating integration error while scrubbing or rendering frames out of order.

Motion profiles should expose perceptual controls and retain physical parameters internally:

```text
product: response 420ms, damping ratio 0.82
camera:  response 620ms, damping ratio 0.88
snap:    response 240ms, damping ratio 0.90
```

Position, scale, rotation, opacity, focus intensity, and camera framing can all use the same trajectory abstraction. Color and discrete content changes may require specialized interpolation rules.

## wgpu Provides the Rendering Substrate

`wgpu` is appropriate because the differentiating effects are temporal and GPU-friendly. It is not a complete 2D renderer, so the prototype must keep its visual vocabulary small.

The first renderer needs:

- Solid and gradient backgrounds.
- Rounded rectangles with subtle borders.
- Monospaced glyph runs with syntax colors.
- 2D transforms and hierarchical clipping.
- Opacity and simple blend modes.
- Camera transforms.
- Offscreen render targets.
- Temporal accumulation.
- Pixel readback for FFmpeg.

The typography spike found that `glyphon` is well suited to ordinary UI rendering but awkward for repeated temporal sampling of independently moving lines. The prototype now uses `cosmic-text` directly to shape and rasterize one cached sprite per stable line. A future GPU compositor can upload those stable sprites without changing the code-transition model. Syntax highlighting can be compiled into styled spans by the authoring frontend, keeping Shiki out of the Rust renderer.

## Motion Blur Samples Real Motion

For output frame `n`, Kinograph should evaluate and render the scene at several times across a virtual shutter interval:

```text
output frame n
    sample scene at t0
    sample scene at t1
    sample scene at t2
    ...
    accumulate samples
    normalize and tone-map
```

Eight samples per frame are enough for the first quality comparison. The renderer can later add adaptive sampling or velocity-buffer blur if render time becomes a problem.

Temporal samples must include camera and layout-driven movement, not just object transforms. This is what makes the effect resemble a camera shutter rather than a directional blur filter.

## Layout Produces Targets, Not Animation

The prototype needs only a constrained 2D layout system:

- fixed size
- center and edge anchoring
- vertical and horizontal stacks
- padding and gaps
- attachment to a line, token, or actor bounds
- camera framing with padding

Layout computes each pose's target rectangles. The motion compiler moves actors between those rectangles. Keeping layout separate from motion avoids accidental animation behavior inside the layout solver.

## Code Animation Extends the effect-institute Model

The existing implementation in `/Users/kit/code/experiments/typescript/effect-institute` has the right semantic foundation. Kinograph should initially port its concepts rather than invent an unrelated code model.

Keep:

- stable line IDs
- stable text outside slots
- named slot values
- snapshot compilation
- line and token focus
- cursor, glow, mark, squiggle, status, and callout annotations
- persistent versus transient annotations
- narration-triggered steps
- stability diagnostics

Change:

- compile snapshots into timed transitions instead of React states
- give every visual part a stable render identity
- make motion profiles explicit and shared
- make camera choreography first-class
- render text, highlights, and annotations through one sampled renderer contract
- evaluate the result at arbitrary timestamps

Defer:

- automatically morphing arbitrary code strings
- AST-aware refactoring animation
- proportional-font source code
- full editor emulation
- terminal emulation

## Prototype Milestones End in Visible Proof

### Milestone 1: Pixels to MP4

Build a Rust CLI that renders a hardcoded frame containing a background, rounded editor panel, and syntax-colored code. Pipe raw RGBA frames into FFmpeg.

Exit criteria:

- `kinograph render hero.json --output hero.mp4` produces a playable 1080p60 video.
- Text positions and colors are deterministic across repeated renders on the same machine.

### Milestone 2: Continuous Motion

Add actors, transforms, analytic springs, sequences, parallel groups, and a 2D camera. Render a panel entering while the camera reframes it.

Exit criteria:

- Redirecting an actor or camera mid-motion preserves velocity.
- Rendering frame 120 directly matches frame 120 from a full sequential render.
- A small trajectory visualizer can display position and velocity over time.

### Milestone 3: Stable Code Transitions

Compile the existing `line`, `slot`, `stack`, focus, and annotation concepts into typed Rust scene values. Animate only changed spans while stable spans retain identity.

Exit criteria:

- A slot transition changes one type signature without making the surrounding code jump.
- Lines move smoothly when another line appears or disappears.
- Focus can target a line ID and a text span.
- Stability diagnostics identify common text unnecessarily placed inside a slot.

### Milestone 4: Temporal Rendering

Render multiple scene samples into an accumulation texture for each output frame.

Exit criteria:

- Fast camera and actor movement show smooth shutter blur.
- Stationary content remains sharp.
- A comparison render can show one, four, and eight temporal samples.

### Milestone 5: Author the Hero Scene

Build the 5-10 second benchmark scene through the least elaborate authoring API that works. Extract repeated operations into the Rust DSL only after the scene exposes them.

Exit criteria:

- The scene contains every element in the first-artifact checklist.
- The source contains no per-frame calculations.
- Timing adjustments happen through cues, durations, and motion profiles.
- The final render looks intentionally choreographed at both full speed and quarter speed.

### Milestone 6: Tighten the Authoring Loop

Add Scene Plan validation and inspection, a persistent renderer process, deterministic plan diffs, single-frame rendering, cue and time-range selection, then file watching and low-resolution preview profiles.

Exit criteria:

- A lightweight Scene Program recompiles without rebuilding the renderer.
- Saving the scene can rerender a selected preview range through the persistent process.
- Compiler errors identify the actor, cue, slot, or target involved.
- The author can inspect a textual list of cues and resolved timestamps.

## The Repository Starts Small

```text
kinograph/
  PLAN.md
  SCENE_PLANS.md
  Cargo.toml
  crates/
    kinograph/           # lightweight authoring, plans, tracks, and validation
    kinograph-render/    # wgpu, typography, FFmpeg, recipes, server, and CLI
  scenes/
    agent-demo/          # lightweight Rust Scene Program
```

The two packages reflect one measured compilation and process boundary. Modules should become additional crates only after another real reuse, versioning, or deployment boundary appears.

## Scope Cuts Protect the Experiment

The completed visual prototype deliberately excluded:

- React, HTML, CSS, or a browser renderer
- a graphical timeline editor
- arbitrary user shaders
- 3D scenes
- audio editing or mixing
- responsive web layout
- plugin APIs
- distributed rendering
- automatic speech transcription
- arbitrary video and image codec support
- real-time full-resolution playback
- feature parity with Remotion or Manim

The next phase reopens audio, transcription, images, video layers, and interactive preview behind the proven Rust DSL. FFmpeg remains responsible for codec work and final media assembly; Kinograph owns source-range edits, timing, scene evaluation, and pixels.

## Risks Have Cheap Tests

| Risk | Earliest useful test |
| --- | --- |
| Rust text rendering cannot match desired typography | Render the final font and representative code before building animation |
| Raw `wgpu` requires too much 2D infrastructure | Limit the hero scene to panels, glyphs, highlights, and simple callouts |
| Temporal supersampling is too slow | Benchmark 1080p with 1, 4, and 8 samples as the first task in Milestone 4 |
| Springs still feel generic | Build the trajectory visualizer and tune against reference clips before adding DSL features |
| Stable code identity becomes cumbersome | Port one complex `effect-institute` animation before generalizing the IR |
| Transcript edits drift out of sync with visuals | Compile both from shared cue ranges on one exact media clock |
| DSL design expands without evidence | Require every new primitive to remove repetition from the hero scene |

## Prototype Success Is Aesthetic and Technical

The prototype succeeds when:

1. The hero scene is visibly better than an equivalent basic Remotion scene in weight, continuity, and blur.
2. Stable code does not jump during authored changes.
3. Camera movement feels like part of the choreography rather than a separate effect.
4. Every frame can be rendered independently and deterministically.
5. The authoring source describes semantic changes without frame math.
6. The implementation remains narrow enough that one person can understand the full pipeline.

It fails if most effort goes into general layout, language syntax, codecs, editor tooling, or framework compatibility before the hero scene exists.

## Immediate Next Step

The lightweight `agent-demo` proves the generic process workflow, and the canonical hero proves a real editor-heavy Rust Scene Program can cross Scene Plan v2 through stable inline parts, logical Semantic Targets, renderer-assisted measurement, continuous pointer/highlight channels, and a concrete editor recipe. Its generated plan and Rust source are checked for byte equality, and the complete 300-frame encoded artifact matched the deleted direct implementation exactly.

The `opencode-session-tool` lesson now proves the narration-rich media seam. One inspectable plan owns an authentic live session creating and hot-reloading a plugin tool, ElevenLabs voiceover, layered SFX, continuous card motion, discrete recording and explanatory-text state, and named cues. The renderer accepts video only when the concrete terminal recipe consumes its media ID, while script and layer audio continue through exact composition and FFmpeg placement.

The next authoring-loop proof is file watching plus a low-resolution preview profile in the persistent renderer. It should rebuild a lightweight Scene Program, validate and diff the generated plan, and render a selected cue without restarting GPU or font state. Raw plan patching remains experimental until that measured loop shows whether whole-plan regeneration is insufficient.
