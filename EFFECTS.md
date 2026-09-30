# Composable Effects

Effects are pure samples of a local clock. A Scene Program owns when and where
they happen; effect Modules own their physical pose and optical response.

## Current library

| Module | Interface | Responsibility |
|---|---|---|
| `kinograph::effects::combustion` | `Burst::sample(age).ember(direction, seed)` | Compression, ignition, rim-light envelope, gravity/drag embers, cooling |
| `kinograph::effects::surface` | `impact(age, angle)`, `wavefront(age)` | Local contact dimple and a damped emissive wave over a sphere |
| `kinograph::effects::spinner` | `sample(age, release, mark, shape)`, `handoff(after)` | The blog's radial spinner: closed-form critically damped motor, speed-driven wake, and a mark route drawn from a top-right handoff |
| `kinograph::math::dynamics` | `ballistic(velocity, acceleration, drag, seconds)` | Closed-form reusable particle displacement |
| `render/effects/noise.wgsl` | `fx_noise3(p)`, `fx_fbm3(p)` | Deterministic 3D noise; caller-owned coordinate transforms |
| `render/effects/combustion.wgsl` | `combustion_volume(pixel, radius, age)` | Domain-warped fire/smoke, emission and absorption; requires noise |
| `render/effects/pressure.wgsl` | `pressure_wave(delta, scale, age)` | Inward pinch and outward refraction; returns displacement and ring intensity |
| `render/effects/rewind.wgsl` | `rewind_envelope(age)`, `rewind_tear(pixel, size, age, amount)`, `rewind_snow(...)` | VHS tape rewind: tracking band and seam tear, 30 fps grain, dropouts, scanlines |

The WGSL Modules have no bindings, texture ownership, entry points, or Stage
identifiers. Concatenate dependencies before the consuming shader. The Stage is
one Adapter: it supplies projected coordinates, draws particles, and applies the
pressure displacement to its HDR scene and bloom textures.

Surface responses start at physical contact. The Stage finds a packet path's
first intersection with the visible orb (`Circle::entry_fraction`), derives the
crossing time from the packet's travel curve, and samples the wave by geodesic
angle. `math::shapes::sphere_ring` supplies a continuous surface trace; the
renderer hides its rear hemisphere. This preserves the attached outline while
the local particle skin deforms.

The Stage's `post.rewind` channel is a local age in seconds: negative is inactive,
zero starts without a cut, and 1.4 seconds returns exactly to the original image.
Animate the age linearly. The port follows the OpenCode blog's `vhsRewind.wgsl`:
one scrolling tape coordinate places a tracking band (tape 0.22) and a thin seam
(0.66); quintic 0.3-second edges give zero slope at both clean endpoints. Snow is
multiplied by an ink mask (luminance above the background), so the empty canvas
stays clean, and its base grain is lighter than the web rig's so lit smoke reads
as tape rather than broadcast snow.

Stage cards carry the blog's deletion vocabulary as ordinary channels: `cool`
(inks toward the frame gray), `damage` (red in one frame), integer `glitch` seeds
(banded horizontal displacement; zero is off), `cut` (0..0.4 a red hairline
draws between title and status, 0.4..1 the halves part 3 px and fade), and
`ghost` (the red outline left in the slot). Every step is a function of its
channels, so the rewind plays the deletion backwards. Banding and parting clip
copies of the card's primitives by their bounding quads; no offscreen target is
needed. The card's `spinner`, `release`, and `mark` clocks drive
`effects::spinner`; its `mark` field chooses a check or a cross.

`Burst` uses seconds and world pixels. Negative age is intact; zero preserves
the shell pose; 5.2 seconds is spent. Direction is a unit vector and each seed
component is in 0..1. Sample any age in any order. Scene/camera transforms and
ambient rotation stay with the caller. Shader color is premultiplied linear HDR.

## Tuning and research

Set `KINOGRAPH_SHADER_DIR=crates/kinograph-render/src/render` to reload shader
sources, including the `effects/` files. Keep short canonical studies under
ignored `output/`; keep the experiment method and decisions in `perf/`.

`python3 scripts/bench-stage-burst.py <label>` records deterministic pixel
continuity, local rim response, and the known volume-boundary regression.
See `perf/stage-burst-quality.md` for the scope of those metrics. Review normal
speed clips, contact strips, late cooling, and rewind before accepting tuning.

Lightning/electricity and water are future effect families. Add each with a
concrete scene and a small study first, composing the noise, curves, motion, and
optics that already fit. Extract a shared solver or new crate when those uses
demonstrate its Interface. Preserve independent timing, spatial coordinates,
seed identity, and deterministic sampling across every composition.
