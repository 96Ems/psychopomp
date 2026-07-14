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

## Next Question

Can these proven, renderer-specific operations compile from a small serializable scene IR without losing their stable identities or physical trajectories?
