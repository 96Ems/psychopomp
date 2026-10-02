# Native interactive showcase

Four independent slides, one persistent Rust renderer:

1. **Effect.succeed:** stable inline reveals.
2. **Task lifecycle:** idle, running, success, failure, and retry.
3. **Parallel work:** independent requests, retrying one branch, then using the results.
4. **Code edits:** add/remove lines while stable code and its highlight move together.

```bash
cargo run -p psychopomp-interactive-showcase
cargo run --release -- plan present target/interactive-showcase/deck.json
```

`'` advances slides; Shift+`'` goes back. Left/Right navigate steps. Keys 1–4 jump
to a slide. Space/P pauses or resumes; R replays the current step. M toggles reduced
motion, X toggles smooth/pixelated display, F toggles fullscreen, and Escape closes.
Each slide remembers its step. Running Tasks continue their energy animation while
you hold a step, but pause and slide departure freeze their local time.

The Effect blocks reuse Psychopomp's Rust port of Effect Institute's
`PixiEffectRow` visuals, including compressed running geometry, jitter/energy
sweeps, result text, error bubbles, and stable labels. This is a native adaptation,
not embedded WebGPU/JavaScript or actual execution of the illustrated Effects.
Task dimensions and state-layer presence are ordinary interruptible tracks.
Content has independent scale/blur/opacity tracks with Effect Institute's Pixi
timings: 200 ms bouncy body compression, roughly 167 ms icon motion, 250 ms result
pop, and 150 ms result deblur. The star contracts and turns while the block
compresses; the sweep no longer waits for the content to leave. Bubble fade,
scale, blur, and rise have their own source profiles rather than a staged wait.
Results resolve without spinning. These tracks reverse smoothly on Left/Right
and apply to video export; no mount/reset behavior is needed on navigation.
The rolling explanation lines move through a fixed window with 12-pixel linear
top/bottom fades, like Visual Types. They disappear behind its edges rather than
floating away as fully visible rows. Their existing movement timings are unchanged.
Error bubbles stay readable while their failed step is held. Running motion uses
a bounded deterministic signal so leaving a Task running for a long presentation
does not increase its sampling cost.
The showcase is silent; it does not yet port every Pixi overlay, connector, or SFX.

The program also emits individual Scene Plans next to `deck.json`:

```bash
target/release/psychopomp plan steps target/interactive-showcase/stable-code-edits.json
target/release/psychopomp plan frame target/interactive-showcase/parallel-effects.json 11 output/parallel.png
target/release/psychopomp plan render target/interactive-showcase/task-lifecycle.json output/lifecycle.mp4
```

Edit `src/lib.rs` to change examples. A Task recipe supplies a name, center,
initial `TaskState`, and timed state changes. The renderer owns text measurement
and visual lowering. Explicit x/y channels can move a Task independently; its
generated width, height, scale, opacity, activity, `state.*`, `content.*`, and
`bubble.*` channels are renderer-owned. See the source timing table in `PRIOR_ART.md`.

Verification:

```bash
cargo test -p psychopomp-interactive-showcase
PSYCHOPOMP_STABILITY_ARTIFACTS=1 cargo test -p psychopomp-render --release -- --ignored --nocapture
```

The GPU checks cover code and Task reversals, deterministic out-of-order pixels,
semantic attachments, multi-step line edits, and running-frame cache invalidation.
Optional code artifacts go under the workspace's `output/stability-proof/`.
