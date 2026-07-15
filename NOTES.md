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
- The complete 31.1-second `promises-only-happy-path` lesson ports without a new renderer feature: two independent inline reveals on one stable call line model `???` being replaced by `throws SomeError`, while the original narration retimes focus, pointer motion, and the optional call line through semantic cues.
- A semantic target after a hidden alternative on the same stable line needs its measured x-position adjusted by that alternative's sampled width. This is the same unresolved layout seam exposed by collapsing slots in `effect-shows-errors`, now reproduced by a second scene.
- Semantic text targets must currently use unique text within their stable line: targeting `ship` selected the earlier substring in `shipment`, while `ship(payment)` resolves the intended call. Occurrence-aware targets remain a future DSL seam.
- Applying one sampled translation, three-axis rotation, and scale to both shader geometry and the CPU-composited foreground produces a coherent 0.7-second perspective entrance whose temporal blur comes from scene motion rather than a post-process blur.
- Extreme perspective entrances expose sampling quality quickly: a five-tap cross reads as repeated glyph copies, and eight shutter samples reveal ghost contours during a 150%-to-100% pullback. A depth-weighted 3x3 Gaussian kernel plus 16 entrance samples produces a smoother near-plane blur while settled frames retain the normal eight-sample cost.
- Published narration bytes can still produce a different browser mix when a lesson flow schedules synthesized sounds separately. The Promise lesson's narration asset is byte-identical to production; its descending E5-to-C5 Tone.js cue must be represented as a cue-local layer clip.
- Effect Task states can be authored as ordinary Composition leaves while stable IDs preserve nodes across idle, running, success, failure, death, retry, and hidden intervals. Arbitrary-time sampling resets entrance age after a hidden interval rather than depending on prior rendered frames.
- Porting Pixi task pixels requires one shared fractional transform for every moving layer. Integer-snapped or independently transformed body, sweep, border, flash, and glow edges visibly separate under subpixel jitter even when their high-level spring targets match.
- Motion's width and height springs are intentionally independent in the Task recipe: running height changes over 0.2 seconds, completed result width over 0.35 seconds, and running scale returns from 0.95 without a one-frame geometry jump.

## Next Question

Can multiple recorded takes be transcribed and assembled into one non-destructive script edit whose changed word timing automatically retimes the same visual choreography?
