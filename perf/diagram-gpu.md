# Bare diagrams: shared GPU / browser proof

## Scope and visual decision

Kit rejected the original dotted card UI: the canvas should contain only labeled
boxes and attached lines, with an optional isometric representation. The canonical
scene is now bare Flat; a second Scene Plan auditions shallow Isometric boxes.
There is no frame, title, caption or step strip, including invisible chrome tracks.
Both views retain the same node/link IDs and continuous motion channels.

The old CPU diagram compositor is replaced by `render/diagram.rs` and
`render/diagram.wgsl`: one bounded GPU pass, a cached R8 glyph atlas, 4× spatial AA,
continuous text/box blur, trace heads and a restrained daemon halo. The final
5×5 binomial node filter removes visibly separated copies in wide entrance blur
without retiming any channel. The diagram
does not use the general CPU card compositor or its backdrop cache anymore.
Isometric is equal-axis orthographic projection with shallow same-plane extrusion,
upright labels and authored paint order—not a general depth-tested mesh scene.

This is a requested visual redesign **and** a GPU migration. Do not call it a
byte-identical CPU-to-GPU optimization or attribute every timing change solely to
the GPU: antialiasing/filtering and the removed chrome also differ. The prior CPU
results remain in `perf/daemon-diagram.md` and `output/daemon-speed/`.

## Shared delivery

The isolated `experiments/browser-grid-prototype` stages the actual native
`render/diagram.rs`, shader and `plan_runtime/diagram.rs`, just as it stages Grid.
The lightweight Scene Program, Timeline, Playback and cancellation-safe Start
Delays are shared. Browser host code only acquires/presents the canvas and supplies
native-baked glyphs. No browser motion implementation, DOM diagram or renderer fork.
Native/export still read back RGBA; the browser directly targets its acquired
sRGB canvas view, with no full-frame CPU readback/blit in ordinary animation.

```sh
cargo run -p psychopomp-opencode-architecture
cargo run --release -- plan present target/opencode-architecture/deck.json --theme original
PSYCHOPOMP_WEB_ASSETS="$PWD/target/browser-diagram-site" bash experiments/browser-grid-prototype/run.sh --build-only
bun experiments/browser-grid-prototype/diagram-proof.ts
python3 experiments/browser-grid-prototype/compare-diagram.py
```

Generated plans: `daemon-merge.json`, `daemon-isometric.json`, `deck.json` under
`target/opencode-architecture/`. The native deck uses 1/2 to select the view.
The browser page is `/diagram.html`; `/` still hosts the existing grid showroom.

## Measurements

All source frames are 1920×1080, one native/browser temporal sample, release.
Exports retain eight shutter samples. No resolution/blur/effect quality switch.

Initial native runs on Apple M2 Max, 2560×1440 window, 60 Hz FIFO:

- Flat: median-round p95 submission interval **18.68 ms**; worker median **7.43 ms**.
  Mostly 59–60 submissions/sec; first measured round had 54.
- Isometric: median-round p95 **18.41 ms**; worker median **8.48 ms**;
  59–60 submissions/sec.
- These are submission timings, not scanout. Occasional pacing spikes remain;
  do not claim a perfect locked 60 Hz or verified 120 Hz.

The final denser-filter runs (`flat-dense.json`, `isometric-dense.json`) measured
**18.50 / 19.88 ms** median-round p95 submission intervals, with worker medians
**8.50 / 8.36 ms**. Flat submitted 59–60 frames/sec; Isometric 57–60. Earlier
repeats also showed p95 variability up to 21.13 ms, retained in the raw reports.
This is substantially smoother than the former CPU diagram, not a locked-60 claim.

The isolated browser test uses Chrome for Testing **151.0.7922.34** on Metal,
never Kit's active browser tab. One warmup and seven measured runs per view,
120 frames/run at an explicit 60 Hz scene clock, navigation every 15 frames,
queue-completion fences every 12 frames. Initial medians:

| View | GPU-completed batch wall ms/frame | MAD | CPU sample/submit ms/frame |
| --- | ---: | ---: | ---: |
| Flat | 2.247 | 0.006 | 0.059 |
| Isometric | 2.773 | 0.004 | 0.055 |

Final 5×5-filter browser results: Flat **3.080 ms** completed batch wall/frame
(MAD 0.151), Isometric **4.864 ms** (MAD 0.043). CPU sample/submit medians were
**0.061 / 0.067 ms**. The denser blur is a visual quality refinement, not a speed
optimization; both the earlier cheaper filter and final measurements are recorded.

Completed batch work is not isolated GPU timestamp time, displayed FPS or scanout.
It excludes initialization and plan loading. Native readback/window work and these
browser batches are different measurements, not a browser-versus-native ratio.
The fresh diagram-only host requests 33,177,600 bytes of MSAA color, 68,096 bytes
of glyph coverage for this seven-label catalog, plus a four-byte placeholder view:
about **31.71 MiB**. Descriptor accounting excludes canvas swapchain/browser/driver
overhead and is not measured physical VRAM. Loading Grid allocates its own larger
resources; multi-embed/resource-sharing and mobile GPU performance remain unproved.

## Pixel and behavior gates

Each view has eight authored and sixteen interrupted native/browser captures.
The first comparison had **44/48 exact diagram PNGs**; the other four differed
in 1–16 components, each by only one byte value out of 8,294,400 components/frame.
The final denser filter has **45/48 exact** captures; the other three differ in
11, 3 and 84 components, each ±1. The 84-component case was inspected: one color
rounding strip on a fading box edge, no alpha differences. The local count gate
was explicitly adjusted from 64 to 128 after this review; the ±1 amplitude limit
is unchanged. This is not a universal cross-GPU tolerance. Both hosts use the
same shader and baked glyphs.
The grid smoke frame also renders; its separate native/browser difference is
28 components, maximum 22, consistent with sparse cross-backend grid differences.

The proof exercises real rAF navigation, pause, exact frame stepping, speed,
theme changes without clock advancement, reduced motion, a stale scene-fetch
race, grid regression and disconnect cleanup. Native tests cover both views'
native/export onset equality, cancelled waits, position/velocity and boundary
pixel continuity through interruptions, plus deterministic out-of-order sampling.
Flat additionally retains the original exact unchanged-client pixel strip test.

The standalone shim intentionally does not host native test-only `PreparedPlan`,
FFmpeg or full-renderer fixtures. Check its native `--lib --bin bake` and WASM
`--lib` targets with strict Clippy; `--all-targets` forces those unrelated staged
native tests into the shim and fails. Full native integration tests belong to the
root workspace. Source files remain staged byte-for-byte rather than edited to
work around that harness boundary.

Raw measurements, captures, logs and recordings live under `output/diagram-gpu/`.
This does not establish aesthetic approval, browser dynamic typography, semantic
canvas accessibility, device-loss recovery, or a production browser API.

Final verification passed: workspace tests/formatting/strict Clippy, native and
WASM builds and strict supported-target Clippy, all **37 native GPU/artifact tests**,
and all **48 browser pixel comparisons** under the documented gate. Both 72-frame
shutter-sampled merge clips and both 132-frame interrupted-navigation clips were
generated; the merge exports and browser recording decode successfully, and
full-scale frames plus stepped strips were inspected. The original native Grid
17-second image remains pixel-identical. The served WASM/HTML were read back and
matched the tested files. No GPU, font or FFmpeg constraint blocked verification;
native scanout remains unmeasured. The native two-view deck and localhost diagram
page are available for Kit's aesthetic review, without driving his browser tabs.
