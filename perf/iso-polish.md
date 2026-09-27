# Isometric side ports and volume entrances

Kit liked the isometric direction, but wanted connections at the middle of the
side faces and more interesting box appearance/entrances. This pass leaves Flat
unchanged and keeps the native/browser shared renderer and motion engine.

The first pass below is historical. The follow-up at the end corrects whole-box
ordering and replaces the competing depth/lift springs with one faster rise.

## Port diagnosis

The original port projected a footprint edge onto the **top** plane. The default
28-unit extrusion therefore needed a 14-pixel side-midpoint offset. A focused
geometry test failed with `[659.4892, 297.5]` instead of `[659.4892, 311.5]`.
The delivered-pixel test also failed: zero changed pixels reached that midpoint.

Correcting the coordinate alone passed the geometry test but **still failed the
pixel test**. All wires were painted behind all boxes, so the visible wire did not
appear until the lower rim. The shader now compares the interpolated wire height
with the sampled top/side-face hit before painting an isometric wire. The midpoint
is visible on an exposed side; the opposite rear-side port remains occluded by an
opaque top. Tests protect both cases rather than painting x-ray connections.

`Pose::port` follows position, footprint dimensions, scale, depth and lift. The
ground datum stays fixed while the box grows upward. Top/Bottom still name the
footprint edges, not horizontal solid faces. Isometric merge connections now
converge on the central port; Flat keeps its prior tangential fan.

This remains a finite diagram compositor. Boxes retain authored paint order;
wire visibility through partially transparent boxes uses stylized attenuation,
not a general physically depth-sorted translucent mesh renderer.

## Appearance and choreography

Only Isometric changes:

- Clearer charcoal top/side faces, restrained lighting gradient, brighter upper
  rim and a subtle purple material tint during daemon emphasis.
- Depth **8 → 48**, **360 ms / 0.22 bounce**.
- Lift **40 → 0**, **280 ms / 0.18 bounce**, including the spring's small overshoot.
- Whole-box scale stays at one; entrance blur is **3.5**, rather than 12.
- The existing **120 ms server wait** applies to opacity, blur, depth and lift.

These are independent authored scalar tracks, not delayed callbacks or another
easing window over a shared progress value. Retained boxes do not replay their
entrance on merge. Interrupted motion carries position and velocity; ports consume
the resulting sampled geometry directly. This is a requested choreography
refinement, not a claim of unchanged reference motion.

## Proof and performance

Artifacts/logs: `output/iso-polish/`. The proof includes the failing tests before
the fix, the offset-only failure, final geometry/visible-port/opaque-occlusion
checks, out-of-order depth/lift/scale cases, normal-speed entrance export, stepped
strip, interrupted movies and native/browser comparisons. All 24 Flat comparison
frames remain pixel-identical to the prior browser capture set.

At unchanged 1920×1080, 4× spatial AA and one native temporal sample, the native
isometric benchmark submitted **58–60 frames/sec**, with **20.29 ms** median-round
p95 submission interval and **6.79 ms** median worker rendering. These are not
scanout measurements or a locked-60 claim.

Isolated Chrome for Testing 151.0.7922.34, one warmup + seven fixed-clock repeats:
The final repeated run measured Isometric completed batch wall time **3.443
ms/frame** (MAD **0.015**), CPU sample/submit **0.063 ms/frame**. Flat measured
2.383 ms completed / 0.065 ms CPU.
They are not isolated GPU timestamps or display FPS, and changed choreography
means this is not a backend-only speed comparison.

All **48 native/browser comparisons** pass the existing ±1 / 128-component gate:
45 exact PNGs and three with one-byte rounding differences. The gate was not
relaxed for this pass. Workspace tests/formatting/strict Clippy, all **38 GPU tests**,
and native/WASM supported-target Clippy and builds pass. GPU/font/FFmpeg did not
block verification; native scanout is not captured.

The browser build is under `target/browser-iso-polish-site`, separate from the
already-open older probe. The review URL uses port 5204 and
`diagram.html?scene=daemon-isometric`. Neither the old 5203 assets nor Kit's active
browser tab are reloaded/replaced. Shader, native bake and WASM still come from the
same shared source modules.

## Follow-up: foreground ordering and a faster critical entrance

Kit liked the appearance but found foreground boxes covered by the middle pair,
and the growing-up / moving-down entrance contradictory. The overlap is visible
in the 9.2-second merge frame, retained in `output/iso-depth/before.png`.

The minimized two-server pixel test initially changed **100,648 components** when
only the declaration order was reversed. The scene's blanket “retained server
last” rule—not the wire endpoint coordinates or opacity alone—caused the stacking
error. Isometric now sorts each complete paint packet by sampled solid-center
camera depth, `x + y + center_z`, along the projection's `(1, 1, 1)` view ray.
The packet keeps its own labels, shell, blur and halo. Authored order breaks only
equal-depth ties, preserving retained ownership when servers coincide; it no
longer beats a genuinely nearer box. Flat's authored order is unchanged.

This is a bounded whole-box painter for the diagram, not exact per-fragment
sorting of arbitrary interpenetrating meshes or translucent solids. Wires retain
their sampled face-height visibility test; no x-ray endpoints are introduced.

The Isometric entrance now uses **depth 8 → 48, 220 ms visual duration, damping
ratio 1** from a fixed base. There is no authored lift channel, downward landing
or bounce. **Blur 6 → 0, 120 ms, damping ratio 1** clears independently, with the
same dense spatial filter as before. Opacity and the server's 120 ms resting wait
remain unchanged. This is requested choreography refinement, not a claim of
equivalent-work speed improvement or unchanged reference timing.

Evidence is under `output/iso-depth/`: the failing order/entrance tests, revised
frames, shutter-sampled entrance and merge clips, and interrupted navigation.
The order regression checks both catalog permutations with the foreground on
either side, opaque and fading/blurred packets, a return to a previous sampled
pose, foreground-label pixels against an isolated box, and retained-label pixels
at coincident depth. It does not merely accept any order-independent sort.
The held Isometric pose remains pixel-identical to the prior styling/layout, and
the Flat Scene Plan is unchanged.

Final verification passed workspace tests, formatting and strict Clippy, all
**39 native GPU/artifact tests**, native/WASM supported-target Clippy/builds, and
the existing **48-case** native/browser gate (**45 exact**, remaining differences
at most ±1; no tolerance change). All **24 Flat browser captures** match the
previous revision exactly. The isolated browser proof also covers controls,
paused frame stepping, themes, reduced motion, the stale-load race and teardown.
Full-scale frames and stepped entrance/merge/interruption strips were inspected;
the generated movies decode cleanly. GPU, font and FFmpeg did not block proof.

The new native interruption run recorded **58–59 submissions/sec**, median-round
p95 interval **20.71 ms**, median worker **7.87 ms**. Isolated browser Isometric
completed batches measured **3.253 ms/frame**, MAD **0.012**, CPU sample/submit
**0.0575 ms/frame**; Flat measured 2.273 ms completed / 0.060 ms CPU. Conditions
remain 1920×1080, 4× spatial AA, one temporal sample, with one browser warmup and
seven measured runs. These are pacing/batch wall times, not scanout or isolated
GPU timestamps. The changed entrance is not an equivalent-work optimization;
no speedup claim follows from these runs.

Updated browser assets are isolated under `target/browser-iso-depth-site`, served
on **5205** at `diagram.html?scene=daemon-isometric`. The served WASM and plan were
read back and matched the tested files. Existing 5203/5204 assets and user tabs
were not replaced or driven. The running native preview was replaced only after
checking its current PID and exact command.

## Follow-up: width formation and client names

Kit asked for width growth too: even with the correct paint order, a newly visible
full-width box occupied its neighbor's space before recentering made room. The
GPU-free test reproduced a projected clearance of **−107.978 units** at 3.003 s.
It checks width, neighbor placement and incoming depth together, not footprint
separation alone: tall side faces can project across a positive footprint gap.

The Scene Program now adds a **340 ms critically damped `width-reveal`** from
`8 / 300` to one. The ordinary sampled width is multiplied by that reveal, without
scaling height or text. The 220 ms depth rise, 120 ms deblur and 120 ms server wait
are retained. Width opens slightly behind depth to maintain at least six units of
projected clearance in both authored pair formations (sampled every millisecond
through their first 800 ms). This is finite authored choreography, not a general
collision-avoidance guarantee for every interrupted or interpenetrating layout.

Separating width presence from the server's structural 300 → 380 merge width
keeps the server's resting entrance delay valid even when skipping directly to
merge; retained boxes keep reveal one rather than replaying an entrance.
`Pose::sample` supplies the same revealed width to geometry and ports. The label
keeps its size and orientation, masked to the sampled top face with a one-pixel
inward feather. The narrow-face GPU test originally found **453 leaking label
pixels**; it now checks both no escaped ink and retained visible ink inside.

Client labels are **TUI 1**, **TUI 2**, and **DESKTOP** in both views. Flat's plan
changes only those labels; its motion is not modified. The source choreography
project remains read-only. Evidence for this pass is in `output/iso-width/`.

Workspace tests, formatting, strict Clippy, all **40 GPU/artifact tests**, and
supported native/WASM Clippy/builds pass. The skipped-merge regression verifies
that width waits until 120 ms and that reversing before onset cancels the wait.
The browser comparison passes the unchanged **48-case** gate: **43 exact**, five
with at most ±1 differences. No tolerance was relaxed. Real controls, reduced
motion, frame stepping, themes, the stale-load race and teardown also pass.

With only the requested names applied to the frozen previous plan, its held
Isometric PNG matches the new held PNG exactly. Flat's entire Scene Plan likewise
matches the previous plan after changing only those three labels. Formation and
interrupted exports were decoded and inspected as full-scale frames/stepped
strips. GPU, fonts and FFmpeg did not prevent verification.

The verified browser revision is isolated under `target/browser-iso-width-site`,
served on **5206** at `diagram.html?scene=daemon-isometric`. Served WASM and plan
read back exactly match tested assets. Earlier review assets/tabs remain untouched.
