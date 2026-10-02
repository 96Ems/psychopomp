# Browser grid / diagram prototype — throwaway

**Question:** can the existing chess grid retain its visual treatment and
interruptible motion inside a browser canvas, without a rendering server?
This is a portability/feasibility probe, not a supported browser library.

The probe now also hosts the bare **Daemon / merge** diagram in Flat and
Isometric views. Open `/diagram.html` for that canvas-only comparison; `/` remains
the original grid showroom. Diagram motion, port sampling, Start Delays and GPU
painting come from the actual native modules, not a second implementation.

```sh
bash experiments/browser-grid-prototype/run.sh
```

Open <http://127.0.0.1:5201>. `PORT=5202` selects another local port. The command
builds a native glyph bake and a release WASM module, then starts a static Bun
server. First run installs the WASM Rust target and a repo-local wasm-bindgen CLI.
Requires Rust, Bun, a native wgpu adapter for baking, and installed CommitMono
(`PSYCHOPOMP_FONT` overrides the default `~/Library/Fonts/CommitMono-400-Regular.otf`).
The browser needs WebGPU on localhost or HTTPS. No browser flags are required.
The Rust `web_sys_unstable_apis` compile cfg is for the pinned Rust bindings,
not a request to disable browser security.

## What is shared

```text
scenes/keyed-grid                    → original two Scene Plans
psychopomp                           → same Timeline, springs, Playback, speed
plan_runtime/grid.rs + children      → same destinations and label disclosure
render/grid.rs + shaders + edges     → same 3D geometry, occlusion, 4× AA, strokes
scenes/opencode-architecture         → same two bare diagram Scene Plans
plan_runtime/diagram.rs              → same validation and cancellation-safe waits
render/diagram.rs + diagram.wgsl     → same boxes, ports, glyphs, blur, projection
prototype host                      → WASM bindings + WebGPU canvas delivery
```

`build.rs` stages byte-for-byte copies of the private grid modules into ignored
Cargo output, using `mod.rs` locations so their relative module/shader paths work.
It declares the source dependencies to Cargo. There is no checked-in renderer
fork or browser-specific grid/diagram implementation.
The prototype has its own Cargo workspace and is not a new production crate split.

The small host retains the names `HeadlessRenderer` and `read_frame` solely to
fit the original private modules. On WASM, the host acquires an RGBA canvas texture
before each sample and the shared passes render straight into its sRGB view.
`read_frame` submits/presents it and returns an empty byte vector: no extra full-frame
texture, blit pass, or CPU readback. Pixel reads are explicit comparison operations,
not normal animation. The shared native/browser label atlas now stores unchanged
8-bit coverage in R8 instead of carrying unused RGB channels.

## What is deliberately missing

- A grid or bare diagram root is drawn. Surrounding native slide overlays are not
  ported; the host exposes the current step title as ordinary accessible HTML.
  The diagram page keeps that description screen-reader-only: no dotted stage,
  enclosing frame, caption or step indicators are painted on the canvas.
- Labels are shaped/rasterized once on the native machine, compressed as a PNG
  plus metadata, and loaded into the existing GPU atlas. **No arbitrary browser
  text, dynamic labels, editor renderer, or runtime font loading.**
- The bake uses locally installed fonts/fallbacks for comparison. Generated
  assets stay under ignored `target/`; font/atlas redistribution rights must be
  checked or replaced with licensed fonts before publishing anything.
- Fixed 1920×1080 source, one temporal sample, 4× spatial AA. CSS scales the canvas.
  No responsive layout, DPR-specific text rasterization, or export shutter blur.
- WebGPU only. Unsupported browsers see a static native reference, not interactive
  WebGL/Canvas2D fallback. Device loss requires reload; no recovery engine yet.
- State is memory-only. Themes and speed are not saved. The element tears down its
  callbacks on disconnect, sleeps at rest, and freezes while hidden/offscreen.
- The named HTML controls and step description are accessible, but the diagram
  itself is rasterized, not selectable/searchable semantic content. This is not
  sufficient accessibility for a shipped educational component.
- Main-thread WASM; no worker, SharedArrayBuffer, cross-origin isolation, or server
  rendering. Multi-component GPU/resource sharing is untested.

## Inspect it

Focus the canvas for arrows, Home/End, R / Shift+R, P, S / Shift+S, and comma/period.
The buttons expose the same actions. Step changes retarget the Rust Playback;
Previous is not a backwards movie. The two scene choices restart their own probe.

The readout distinguishes rAF intervals and CPU sample/submission cost from GPU
time and display scanout. The WASM heap excludes GPU textures and browser memory.
`window.probe.sample(seconds)` explicitly samples the authored Timeline and
returns a PNG for comparison. Any normal navigation returns to interactive mode.

## Cost questions

- Hosting is static assets/CDN bandwidth, not a per-view rendering server.
- WASM adds download/compile/startup and JS↔WASM bindings. Reusing Rust avoids
  maintaining another physics/layout engine; it does not inherently improve pixels.
- WebGPU adds GPU work, memory, compatibility, and device-loss handling. Those
  costs exist whether the caller is Rust/WASM or TypeScript.
- Autoresearch reduced explicitly requested texture storage from ~168.7 to
  **~152.9 MiB including the atlas**, before the canvas swapchain, browser overhead
  or driver allocation strategies. This is descriptor accounting, not physical
  VRAM. The quality-first recipe still needs mobile/multi-embed measurement;
  the WASM heap is not its full memory footprint.
- Build-time glyphs reduce runtime dependencies but move typography into an asset
  pipeline. Live editable code is a materially bigger project than prepared diagrams.

## Verdict

Browser comparison succeeded and Kit liked the two prepared grids. See `NOTES.md`
here for payload, pixel differences, timing, GPU memory,
and verification limits. Absorb only a validated seam or remove this prototype;
do not publish the experimental bindings as an API.

## Repeat the performance work

```sh
PSYCHOPOMP_WEB_ASSETS="$PWD/target/browser-perf-site" bash experiments/browser-grid-prototype/run.sh --build-only
cd experiments/browser-grid-prototype
bun install
bun run bench current
```

The benchmark uses a separate headless Chrome/Brave process and ephemeral profile,
not the user's live tab. `CHROME` overrides the executable. `BENCH_COMPARE` points
to a frozen asset directory for alternating candidate/control runs in one origin.
Use `python3 compare.py ../../output/browser-perf/<label>` (Pillow) to enforce
exact pixel equality. See `perf/browser-grid.md` for fixed conditions, measured
results and discarded hypotheses. `completed()` is a benchmark-only GPU fence;
normal playback never waits for it per frame.

## Repeat the diagram proof

The bake's GPU-free case-inventory test and the capture-receipt tests run separately
from the root workspace:

```sh
cargo test --locked --manifest-path experiments/browser-grid-prototype/Cargo.toml
python3 -m unittest discover -s experiments/browser-grid-prototype -p 'test_*.py'
```

The staged library's native integration tests remain in the root workspace, not
this shim. For strict Clippy, select native `--lib --bin bake` and WASM `--lib`;
`--all-targets` pulls unsupported native test-only imports into the staged library.

```sh
PSYCHOPOMP_WEB_ASSETS="$PWD/target/browser-diagram-site" bash experiments/browser-grid-prototype/run.sh --build-only
bun experiments/browser-grid-prototype/diagram-proof.ts
python3 experiments/browser-grid-prototype/compare-diagram.py
```

The proof uses a disposable headless Chrome/Brave process, never the user's tab.
It captures both views at eight authored times and sixteen interrupted samples,
checks real rAF controls, theme clock stability, stale scene-load cancellation,
teardown and the existing grid. It separately measures GPU-completed batches with
one warmup and seven repeats. These are not scanout or isolated GPU timings.
`perf/diagram-gpu.md` records native/browser findings. A diagram-only host requests
one 1080p 4× color attachment plus a small R8 atlas; the older ~152.9 MiB grid
texture figure does not describe the diagram.
