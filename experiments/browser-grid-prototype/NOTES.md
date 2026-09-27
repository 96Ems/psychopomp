# Browser grid probe findings — 2026-09-04

## Answer

**Follow-up:** Kit liked the browser result and requested shared-code performance
autoresearch. The retained changes are direct RGBA canvas delivery (remove a full
frame texture/blit) and R8 label coverage shared with native. Explicit texture
requests fell **168.7 → 152.9 MiB (−15.8 MiB / −9.4%)** with unchanged 1080p/4×AA
and **27/27 byte-identical same-session frames**. Throughput gains were modest and
variable; do not promote the old rAF metrics into a new FPS claim. Two speculative
optimizations were discarded. `perf/browser-grid.md` records the loop and scope.
The following sections retain the initial proof's historical measurements.

**Yes for prepared diagrams.** The existing grid recipe runs through WASM +
WebGPU with very close native pixels and the same interruptible Playback. This
does not prove a complete browser runtime, mobile performance, live code rendering,
or a production API. No native renderer, choreography, or motion code was changed.
Keep the isolated prototype for visual review, then absorb a validated seam or
delete it instead of maintaining a parallel product.

## Conditions

Chrome 152, this Mac's Apple M2 Max; 1920×1080 render target, 1280×720 CSS display
in a 2560×1273 viewport. Four-sample spatial AA, one temporal sample. No export
shutter blur, Safari/Firefox, mobile, or 120 Hz claim.

## Payload

| Asset | gzip bytes |
| --- | ---: |
| WASM | 259,743 |
| wasm-bindgen JS | 13,125 |
| Baked labels/metadata | 29,825 |
| Growing-grid Scene Plan | 1,996 |
| **Interactive core** | **304,689 (~298 KiB)** |

HTML/host JS add a few KiB. The demo also eagerly downloads a 91,005-byte PNG
reference/fallback (~89 KiB), including on WebGPU browsers. This is not hidden in
the core-size number. Encoded sizes were read via browser resource timing.
No wasm-opt/Brotli optimization was attempted. No rendering server runs.

## Pixel evidence

- Original production `plan frame` at 17 seconds and the native bake's grid-only
  frame were byte-identical, checking the small glyph-provider shim.
- Six growing-grid samples (0, 3.2, 9.2, 17, 21.2, 23 seconds) covered held poses,
  growth, rotation and cutaway. Browser/native differences: 246–351 RGBA components
  out of 8,294,400/frame, predominantly ±1 byte. Sparse edge pixels reached 22:
  very close, not byte-identical.
- Three regrouping samples (0, 3.2, 9.2 seconds): 244–296 differing components,
  maximum 22. Full-scale representative frames were inspected.
- A shared 16-sample native/browser interruption script covered forward, reverse,
  first/last skips, A→B→A redirects, and before/at/after an interruption on the same
  explicit clock. Differences: 232–519 components/frame, mostly ±1, sparse edge
  cases up to 40. These pixel checks—not only endpoints/FPS—support the narrow
  tested continuity claim. Cross-GPU bit determinism is not established.
- Theme round-trip restored the same browser PNG; Pure Black corner was exactly
  `[0,0,0,255]`. Named HTML controls exercised speed and navigation.
- A CDP browser recording (2560×1273, 362 captured frames, ~14.5 seconds) was fully
  decoded and inspected as a strip. It shows growth/rotation/cutaways, not a
  fixed-FPS export or performance result. Later navigation was user-operated;
  the recording is not an exclusive automated tour of every step. Automation
  stopped for user review.

Evidence: `output/browser-grid/`; native references and shared interruption script:
`target/browser-grid-site/`. Pixel comparisons and metrics are saved as JSON.

## Timing and memory

Seven seconds of last / previous / last / first retargets every 250 ms, normal
speed, visible canvas, same 1080p target. After our builds/checks finished, 391
moving intervals were measured (the host excludes the first interval after each
command):

- rAF interval p50 **16.7 ms**, p95 **17.5 ms**.
- CPU sample/submit p50 **1.0 ms**, p95 **2.6 ms**.
- WASM linear memory **16,056,320 bytes (~15.3 MiB)**.
- Local reload init **85.9 ms**, first submission **131.2 ms** after host-module
  startup. Not controlled cold-cache/network measurements.

These are callback/CPU measurements, **not GPU completion or display scanout**,
and not a native-versus-browser benchmark. Navigation compilation is outside the
sample/submit timer; rAF stalls can reflect it. An earlier run during build work
had p95 66.7 ms and is retained separately, not used as a comparable baseline.
Shared machine load was not controlled.

The WASM heap is not the full footprint. The 1080p color/depth/float-coverage and
resolve/output textures nominally total **~158 MiB**, before labels (~10.5 MiB
here), canvas swapchain, and browser/driver overhead. Actual physical allocation
was not measured; drivers may alias/discard attachments. GPU time, energy, mobile
and many simultaneous embeds remain untested.

## Recommendation

Keep ordinary site text/controls in the DOM. A lazy-loaded canvas component is
credible for prepared diagrams. Costs are delivery, client GPU/battery and
engineering, not server render minutes. WASM reuses Rust; WebGPU owns drawing cost.
Calling the same GPU passes from TypeScript would not make them free.

Before a library: choose the supported content boundary, evaluate adaptive/lower
resolution and mobile browsers, provide semantic/accessibility content, handle
device loss, and resolve font/atlas redistribution rights. Dynamic code text adds
a separate font-shaping and interaction problem. Kit liked the initial visuals;
API promotion and production support remain undecided.

## Checks

Workspace tests, formatting, strict workspace Clippy and diff checks passed.
Standalone native bake and WASM library passed strict Clippy; release builds
succeeded. No GPU/font/browser constraint prevented this scoped comparison.
One Browser Control agent-script host/page scope error was recovered without
resetting the page and recorded/resolved in Organizer.
# Bare diagram follow-up

The probe now stages the actual native bare diagram renderer, WGSL and plan
adapter in addition to Grid. `/diagram.html` offers Flat / Isometric versions of
Daemon / merge with no dotted stage, enclosing UI or on-canvas caption. Both use
the same Scene Programs, node/port identity, Timeline, Playback and Start Delays.
`GridCanvas` retains its historical experimental binding name but accepts either
root. This is not a promoted browser API or general composition port.

`perf/diagram-gpu.md` and `output/diagram-gpu/` record the isolated Chrome for
Testing 151 proof, 48 authored/interrupted comparison captures, real rAF controls,
scene-load cancellation, memory accounting and native/browser timing limits.
The original Grid implementation remains unchanged; its native 17-second pixels
still match the original production reference. Do not apply the historical Grid
GPU-memory number below to a fresh diagram-only canvas.
