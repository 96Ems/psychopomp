# OpenCode architecture · Daemon / merge

A bare Kinograph adaptation of `opencode-architecture`'s four-step `Merge.tsx`:
**only labeled boxes and attached wires**. The original dotted stage, enclosing
frame, heading, caption and indicators were explicitly rejected and are not drawn.
Flat and isometric views share identities, layout and the GPU recipe. Isometric
adds its own width/depth entrance through the same motion engine. Kit likes its styling;
Flat remains the default rather than being replaced automatically.

```sh
cargo run -p kinograph-opencode-architecture
cargo run --release -- plan present target/opencode-architecture/deck.json --theme original
```

**1 / 2** selects Flat / Isometric. **← / →** steps, **Home / End** first/last, **R** replay, **P** pause,
**S / Shift+S** speed, **, / .** frame-step, **Shift+R** replay paused, **D** debug.
**T** changes the native theme; Original preserves this reference's black/purple
palette. The explicit startup theme does not overwrite saved preferences.

1. **TUI 1** and its server, centered.
2. **TUI 2** and server; retained content recenters.
3. **DESKTOP** joins: the screenshot's three-client / three-process pose.
4. Clients remain still while server boxes converge into the wider purple daemon.

## One source, native presentation and export

```sh
cargo run --release -- plan frame target/opencode-architecture/daemon-merge.json 8 output/daemon-merge/three-servers.png --theme original
cargo run --release -- plan render target/opencode-architecture/daemon-merge.json output/daemon-merge/merge.mp4 --range 9..10.2 --theme original
```

`src/lib.rs` owns the step descriptions, semantic identities, layout destinations and every
track. The provisional `prototype-diagram` recipe owns only a finite bare
surface, labeled boxes, attached straight wires, trace heads, and painting.
It has no OpenCode state machine and no automatic graph layout. Its current
styling/geometry vocabulary is intentionally narrow, not a promoted diagram API.

Nodes and links retain IDs throughout. Wire endpoints derive from sampled box
position, width, height and scale. Isometric resolves the **side-face center**
halfway down the current depth, including its lift; merged connections use one
central port rather than the flat view's tangential fan. They
do not follow independent endpoint springs. Isometric paints complete boxes
back-to-front by sampled solid-center depth along the view ray, not screen Y.
The retained middle server wins only equal-depth ties, rather than covering nearer
boxes throughout convergence. Flat retains its authored order. The label swap is a fade-through with no simultaneously readable
payloads. Labels remain upright in Isometric. Step titles live in the player and
HTML accessibility text, not in the rendered canvas; there are no caption tracks.

Isometric uses clearer charcoal face shading and a lit upper rim. New boxes grow
from depth 8 to 48 on a **220 ms critically damped spring**, anchored to a fixed
base. The earlier downward lift and bounce are removed: there is one clear rise,
not two opposing motions. Whole-box scale stays at one; a brief 6-unit sampling
blur clears on its own **120 ms critically damped spring**. The server's 120 ms
resting start delay is preserved on opacity, blur, width reveal and depth. Retained boxes do not replay these
entrances. This is a requested choreography refinement, not source-timing parity.

New Isometric boxes also open sideways from approximately 8 to 300 units on a
**340 ms critically damped width reveal**. The width follows slightly behind the
220 ms rise so the volume's side faces stay clear while adjacent boxes recenter
over 450 ms. It is not another callback or delayed start. Upright labels keep
their font size and disclose inside the sampled top face with a one-pixel inward
feather, instead of floating outside the narrow entrance or stretching with it.
`width-reveal` is independent of the server's structural 300 → 380 merge width,
so skipped steps retain the same 120 ms server wait and retained boxes do not
replay their entrance during merge. The footprint and attached ports use the same
sampled width. Flat gets the new labels but keeps its existing motion.

The port correction has a pure geometry regression and a delivered-pixel test:
correcting the midpoint alone still failed because the old paint order obscured
it. Depth-aware wire visibility fixes the exposed side while keeping rear ports
behind opaque top faces. It is not an x-ray treatment or a new generic 3D scene.

The native player uses ordinary Playback and its cancellation-safe Start Delays.
The Scene Program also offsets authored events by the same waits for export.
No callbacks, mount resets, or frame-history integration are introduced.

## Reference and adaptations

Read-only source: `/Users/kit/code/open-source/opencode-architecture`, HEAD
`b7e0fa8`; `Merge.tsx` SHA-256
`a3a63d46d8f995da934f6f3018f385a7c301190940d51ac8183bd79aa20c533a`.
Also inspected `Card.tsx`, `CyclingCaption.tsx`, `Comet.tsx`, `shared.ts`,
`effects/entrance.tsx`, and the working-tree `styles.css` (already modified).
Nothing in that project was edited or restarted.

The reference's 880×240 SVG content is doubled into a centered 1760-pixel-wide
layout on Kinograph's 1920×1080 canvas. Labels, order, palette and box/wire timing
are retained. The reference-card chrome and rolling caption are intentionally gone.

- 450 ms spatial / 320 ms merge springs, zero bounce.
- Flat: 300 ms scale settle with 0.1 bounce; 140 ms focus/brightness resolution.
  Isometric uses the fixed-base entrance described above instead.
- 120 ms server entrance delay; 60 ms wire-start delay.
- Side-shell / ink fades: 220 ms, waiting 100 / 80 ms on a resting merge.
- 220 ms halo delay; no on-canvas caption roll.
- CSS duration fades are adapted to fast zero-bounce springs. The reference's
  400 ms cubic-ease wire draw becomes a snappy 200 ms visual-duration spring.
  Trace brightness follows that same sampled draw, including reversal.
- The halo uses a smooth distance-based falloff, not SVG filter parity or the
  original timer-driven flash peak. GPU blur retains continuous sampled offsets,
  independently of the unchanged scalar tracks. GPU antialiasing/filtering is not
  claimed byte-identical to the removed CPU card compositor.
- Hidden new pairs rest at their normal arrival positions. Skipped/interrupted
  navigation retains current state rather than copying browser mount resets.

The V1/V2 step descriptions reproduce the authored teaching example; this scene is not an
audit of every supported historical OpenCode launch/attach configuration.

## Scope

The diagram is currently an exclusive native/export root, like Code or Grid.
Both views are also included in the isolated browser probe at `diagram.html`.
The payload supports at most 16 nodes and 32 links; text is single-line CommitMono.
No HTML, remote assets, editable graph, routing engine or arbitrary nested actors.

Targeted evidence lives in `output/daemon-merge/`. Tests cover validation, sampled
ports, native/export onset equality, retained client pixels, cancelled waits,
all-channel position/velocity continuity, interrupted pixels and out-of-order
sampling. A newly exposed shared-card edge case has its own regression test:
zero-width borders must not paint transparent bounds at fractional positions.

## Performance checks

`perf/daemon-diagram.md` records the earlier CPU performance work; the shared GPU
follow-up is in `perf/diagram-gpu.md`. GPU glyph coverage is cached in one R8 atlas.
Four-sample spatial AA, box surfaces, wires, text, blur and halos now render on the
GPU. Native presentation/export still read back RGBA; browser canvas delivery does
not. No CPU backdrop or node-pixel rasterization remains in this recipe.
Run benchmarks separately from builds and other rendering workloads:

```sh
target/release/kinograph plan present target/opencode-architecture/daemon-merge.json --theme original --benchmark
KINOGRAPH_DIAGRAM_PERF="$PWD/output/daemon-speed/check" cargo test -p kinograph-render --release diagram_sampling_benchmark -- --ignored --nocapture --test-threads=1
```

The first measures native submission pacing. The second measures a fixed-clock
one-sample recipe workload and optionally saves exact PNGs; neither is a scanout
or isolated GPU timing measurement. Resolution, filters and choreography are fixed.

## Browser

```sh
KINOGRAPH_WEB_ASSETS="$PWD/target/browser-diagram-site" bash experiments/browser-grid-prototype/run.sh --build-only
PORT=5203 KINOGRAPH_WEB_ASSETS="$PWD/target/browser-diagram-site" bun experiments/browser-grid-prototype/serve.ts
```

Open <http://127.0.0.1:5203/diagram.html>. The experiment stages the actual
`render/diagram.rs`, `render/diagram.wgsl`, and `plan_runtime/diagram.rs`; it does
not maintain a browser renderer fork or another animation clock. Glyphs are baked
locally, so dynamic browser typography and font redistribution remain out of scope.
The isometric view is equal-axis orthographic projection of shallow same-plane
boxes, with sampled whole-box depth ordering, not a general depth-tested mesh scene.
`perf/iso-polish.md` records the side-port and entrance refinements and their follow-up.
