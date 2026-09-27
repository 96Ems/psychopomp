# Effect.succeed, one change at a time

A seven-step code-reveal presentation adapted from Effect Institute's
`content/basics/index.ts`, section `effect-succeed`.

The demo preserves the source's content-changing sequence:

1. `const magicWord = ...`
2. Wrap the placeholder in `Effect.succeed(...)`.
3. Replace only `...` with `"pismire"`.
4. Reveal `: Effect.Effect<string>`.
5. Reveal the error parameter, `never`.
6. Reveal the requirements parameter, `never`.
7. Collapse both trailing `never` parameters.

The declaration is reflowed onto two stable lines to fit the fixed canvas. The
import, binding, equals sign, and indentation never enter changing ranges. All
variable parts use the published 0.4-second, zero-bounce width/opacity/blur motion.
Cursor-only steps, celebration, and narration are deliberately omitted from this
interaction proof. This is an adaptation, not a timing-identical published lesson.

## Try it

From the workspace root:

```bash
cargo run -p kinograph-effect-succeed-slides -- target/effect-succeed-slides.json
cargo run --release -- plan present target/effect-succeed-slides.json
cargo run -- plan steps target/effect-succeed-slides.json
```

The first command compiles the lightweight Scene Program. The second opens a
native Rust window using winit and a wgpu surface. Kinograph prepares the scene once
and renders requested poses directly on a worker. No browser, pre-rendered clips,
server, network connection, or FFmpeg is needed for this code-only demo.

- **Right / Left:** animate toward the next/previous step. Press either during a
  transition to redirect it immediately, preserving position and velocity.
- **R:** explicitly restart the current step from its authored entry pose.
- **Space / P:** pause/resume; Space advances when already held.
- **Home / End:** animate toward the first/last step.
- **X:** toggle smooth scaling (default) and deliberately pixelated scaling.
- **M:** toggle reduced motion, or start with `--reduced-motion`.
- **F / Escape:** toggle full screen / close the window.

The window title shows the destination, playback state, and display filter. Losing
focus pauses motion. Window resizing letterboxes the authored canvas; it does not
reflow code. Smooth scaling removes nearest-neighbor blockiness but does not
replace future higher-DPI glyph rasterization.

Sampling follows the current display's reported refresh rate, rather than a fixed
60 fps cap. Moving to a 120/144 Hz display updates the cap; unknown refresh falls
back to 60 Hz. `--fps 120` explicitly overrides it. FIFO presentation stays enabled,
so requesting 120 fps on a 60 Hz screen cannot produce 120 visible frames/sec.

The default live preview caches stationary editor chrome and paints code directly
over it, without the export compositor's final optical resampling of glyphs.
Camera, visible pointer, and annotation combinations fall back to the full
renderer. `--full-quality` disables this preview shortcut; it still samples one
instant rather than an export shutter. It can be much slower.

See [Play as a presentation](../../SCENE_PLANS.md#play-as-a-presentation)
for the current player capabilities and limitations.

## Export the same source as video

```bash
cargo run --release -- plan render target/effect-succeed-slides.json output/effect-succeed.mp4
```

This plays the original 14-second timeline, including its authored holds. User
waiting time never becomes part of that timeline. Other Scene Plans' normal video
exports retain their audio; the native player is currently silent.

## Verification

```bash
cargo test -p kinograph-effect-succeed-slides
cargo test -p kinograph playback
cargo test -p kinograph-render --release -- --ignored --nocapture
```

The Rust checks cover stable identities, token spans, settled channel values,
mid-flight reversal, velocity continuity, unchanged-target stability, pause/resume,
reduced motion, and deterministic out-of-order trajectory sampling. Ignored GPU
tests compare rendered pixels across interruption and measure live sampling costs.

`plan steps` shows changed-part markers and common-text stability warnings without
a GPU. The demo has no warnings. For multiple slides, Task animations, and keyed
line edits, see `scenes/interactive-showcase/README.md`.

For a repeatable ten-second native frame-pacing benchmark:

```bash
target/release/kinograph plan present target/effect-succeed-slides.json --benchmark
```

The benchmark window stays on top and exits automatically. Do not resize it.
See `perf/native-playback.md` for the workload, measurements, and interpretation.

Use `--fps 120 --benchmark` to test a 120 fps sampling target. Use
`--fps 120 --benchmark-gpu` to measure scene sampling plus upload/draw completion
separately from drawable acquisition. The latter intentionally waits for the GPU
each frame; compare its work budget, not its pacing, with normal playback.

The source project's `bun narrate steps -c basics effect-succeed` was attempted,
but its current installation could not resolve `effect/unstable/cli`. The slot
changes were checked against the source above without changing that project's
dependencies.
