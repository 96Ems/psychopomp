# Prototype Findings

## Question

Can a minimal headless Rust stack render attractive, stable technical-video frames and encode them without a browser?

## Answer

Yes. The prototype renders a three-second 1920x1080 video at 60 fps on an Apple M2 Max, reads frames back from Metal, and streams raw RGBA pixels to FFmpeg. The current 180-degree shutter render evaluates eight complete temporal samples per frame and finishes in about 25 seconds.

The output uses:

- a fullscreen WGSL shader for a flat, gradient-free dark editor and focus treatment
- `cosmic-text` for shaped, syntax-colored CommitMono line sprites
- stable line IDs and compiled before/after code snapshots
- analytic damped springs for panel movement and focus intensity
- separate layout and content progress so space opens before new lines enter
- eight complete scene samples per output frame for shutter-based motion blur
- an offscreen `Rgba8UnormSrgb` texture with GPU-to-CPU readback
- FFmpeg and `libx264` for a deterministic 1080p60 H.264 artifact

Run the prototype with:

```bash
cargo run --release
```

The default artifact is `output/kinograph-prototype.mp4`.

## What This Proves

- `wgpu` works headlessly through Metal without a window or browser.
- `cosmic-text` produces crisp CommitMono typography with per-span styling.
- Cached line sprites avoid dynamic glyph-atlas corruption and make opacity deterministic.
- Stable closing lines move to new rows without being replaced when code enters.
- GPU rendering and synchronous readback are already fast enough for a short offline-rendered prototype.
- Temporal sampling blurs moving geometry and text while settled code remains crisp.
- FFmpeg can remain a narrow encoding boundary rather than part of the rendering engine.
- Arbitrary-time analytic motion fits frame-independent rendering.

## What This Does Not Prove

- camera choreography
- a scene IR or TypeScript authoring frontend
- GPU composition of cached line sprites
- fast interactive preview

## Subsequent Findings

- Shaped glyph cluster hitboxes provide exact semantic token targets without estimating monospace advances.
- Token highlights and an independently moving pointer can target those ranges through ordinary property tracks.
- The exact Effect Institute Phosphor hand can be loaded through a reusable SVG sprite pipeline, transformed per temporal sample, and accumulated with motion blur.
- A stable line can reveal `, NotFound` by expanding the inserted spans, resolving opacity and blur, and moving the existing `>` suffix without replacing it.
- A pure Rust DSL can preserve semantic text targets until scene compilation and lower typed actor operations into the same scalar property tracks.
- Cross-media sequence, parallel, delay, and hold can schedule visual motion together with non-destructive audio or video clips.
- Transcript-bearing script clips and accompanying layer clips require distinct roles even though they share the same composition clock.
- Integer-nanosecond media time avoids source-range drift that appears immediately when decimal clip boundaries are repeatedly subtracted as floating-point values.
- The complete 31.7-second Effect Institute `effect-shows-errors` lesson can use its original Opus narration and word timing sidecar to drive structural code changes, focus ranges, pointer motion, multiple inline reveals, an error squiggle, and a celebration burst.
- FFmpeg can trim and place compiled audio clips on the composition clock while continuing to receive raw rendered video through stdin.
- A separate success sound can be scheduled as a layer clip at the same transcript cue that clears the error and reveals `VeryBadRoll`.
- Semantic targets after a collapsing inline slot must resolve against the sampled slot layout, not the backing line containing every slot alternative; otherwise highlights inherit the hidden span's width.
- Porting an Effect Institute flow requires its operation-specific motion profiles: 0.5-second bouncy cursor travel, 0.4-second critical inline parts, 0.45-second line movement, 0.3-second spotlight changes, and a 0.4-second burst entrance.
- Short semantic annotations can be first-class composition leaves: scheduling resolves their target once, arbitrary-time sampling derives normalized phase without retained particle state, and a closed recipe enum swaps prismatic bloom for focus pulse without exposing renderer plugins or drawing parameters.
- Per-clip decibel gain lets a quiet sound effect remain an immutable source asset while cue-local parallel composition synchronizes it with a semantic annotation.
- Mix level alone does not predict whether a sound effect reads under narration: the original narrow 1 kHz success tap remained masked after an 18 dB boost, while a quieter upward stereo chime with high-frequency transients stays distinct from speech.

## Next Question

Can multiple recorded takes be transcribed and assembled into one non-destructive script edit whose changed word timing automatically retimes the same visual choreography?
