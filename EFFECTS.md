# Composable Effects

Effects are pure samples of a local clock. A Scene Program owns when and where
they happen; effect Modules own their physical pose and optical response.

## Current library

| Module | Interface | Responsibility |
|---|---|---|
| `psychopomp::effects::combustion` | `Burst::sample(age).ember(direction, seed)` | Compression, ignition, rim-light envelope, gravity/drag embers, cooling |
| `psychopomp::effects::surface` | `impact(age, angle)`, `wavefront(age)` | Local contact dimple and a damped emissive wave over a sphere |
| `psychopomp::effects::shake` | `rumble(time, trauma)` | Squared-trauma camera rumble from two octaves of smooth noise: offset and roll |
| `psychopomp::effects::shake` | `handheld(time, amount)` | A held camera's slow sway: two octaves of half-hertz noise on pan, yaw, and pitch, each axis on its own phase; zero amount is exactly still |
| `psychopomp::effects::combustion` | `shock_arrival(distance)` | When the burst's pressure front reaches a distance, mirroring `pressure.wgsl` |
| `psychopomp::effects::spinner` | `sample(age, release, mark, shape)`, `handoff(after)` | The blog's radial spinner: closed-form critically damped motor, speed-driven wake, and a mark route drawn from a top-right handoff |
| `psychopomp::effects::lightning` | `Discharge::new(strikes, seed)` (`leader`, `stroke`, `glowing`, `core`, `afterglow`, `light`, `roll`), `bolt(from, to, roll, branching)`, `jag`, `twig`, `crackle(time, intensity, seed)`, `hum(time, intensity, seed)`, `spark(seed, k, index, normal, since)` | Stepped leader, strobing return strokes, seeded branching bolts, outline crackle, sustained arcs, contact sparks |
| `psychopomp::effects::dissolve` | `field(local, half, seed)`, `front(age)`, `passes(value)`, `ash(index, seed, half, age)` | A noisy burn front over a card and the ash it sheds, timed by the field the shader evaluates |
| `psychopomp::effects::shield` | `cell(up, seed)`, `flare(age)` | A forcefield's seeded cell raise and the light a contact casts |
| `psychopomp::math::dynamics` | `ballistic(velocity, acceleration, drag, seconds)` | Closed-form reusable particle displacement |
| `psychopomp::math::random` | `lattice_noise2`, `lattice_fbm2` | Integer-hashed 2D value noise, bit-identical in WGSL (`fx_lattice2`) |
| `psychopomp::math::optics` | `refraction_offset(slope, ior)`, `superellipse_slope(t, power)` | Snell's sideways bend under a tilted glass surface; the slope of a rounded rim from vertical to flat |
| `psychopomp::lens` | `Glass::source(point, spread)`, `bend(t)`, `bounds()` | A loupe's page mapping: even magnification on the flat top, inward rim refraction, per-color spread |
| `render/effects/noise.wgsl` | `fx_hash2(p)`, `fx_noise3(p)`, `fx_fbm3(p)` | Deterministic 3D noise; caller-owned coordinate transforms |
| `render/effects/combustion.wgsl` | `combustion_volume(pixel, radius, age)` | Domain-warped fire/smoke, emission and absorption; requires noise |
| `render/effects/pressure.wgsl` | `pressure_wave(delta, scale, age)` | Inward pinch and outward refraction; returns displacement and ring intensity |
| `render/effects/rewind.wgsl` | `rewind_envelope(age)`, `rewind_tear(pixel, size, age, amount)`, `rewind_snow(...)` | VHS tape rewind: tracking band and seam tear, 30 fps grain, dropouts, scanlines; requires noise |
| `render/effects/lightning.wgsl` | `lightning_channel(d, half, blur, core, corona, radius, reach)` | Plasma optics: white-hot core, hot sheath, Lorentzian corona; pure emission |
| `render/effects/dissolve.wgsl` | `fx_lattice2(p, salt)`, `dissolve_field(local, half, seed)`, `dissolve_burn(field, age, aa)`, `dissolve_glow(rim, tone)` | The burn mask: scorched band, hot rim, and the integer noise mirrored by `lattice_noise2` |
| `render/effects/shield.wgsl` | `shield_bubble(q, radius, up, time, blur, tone, contacts...)` | Stereographic hex cells, Fresnel limb, raise flashes, `surface::impact` ripples; requires noise |
| `render/effects/scan.wgsl` | `scan_light(p, half, corner, line, heading, wake, width, blur)` | A scan line over a rounded panel: overhanging line, wake, and the rim it crosses |

The WGSL Modules have no bindings, texture ownership, entry points, or Stage
identifiers. Concatenate dependencies before the consuming shader. The Stage is
one Adapter: it supplies projected coordinates, draws particles, and applies the
pressure displacement to its HDR scene and bloom textures.

The Lens is the first optic over any root rather than the Stage: a CPU pass
over the composed frame, in linear light, in `render/lens.rs`. Its rim follows
a superellipse of power 3, so the flat top meets it without a crease and the
bend concentrates near the edge (reaching 1.118 page depths at a vertical face
in crown glass). Restraint is the tuning rule: the middle is a pure even
enlargement (Keys cubic, sharpening with magnification and clamped against
halos), dispersion is 4% of the rim bend and vanishes on the flat top, and
light is a hairline, not a glow. On dark pages the glass reads through its
specular line, sheen, and what crosses the rim; tune against moving text, since
the rim's compression is what crawls if it is under-softened.

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

A Stage `form` takes the same `burst` and `shatter` as an orb: embers leave each
point along its direction from the form's center, and the volume and pressure
wave size to its bounding radius. The first active orb or form drives the
composite's wave. `effects::surface` is spherical, so a form takes no contact
ripple; it pulses instead.

`Burst` uses seconds and world pixels. Negative age is intact; zero preserves
the shell pose; 5.2 seconds is spent. Direction is a unit vector and each seed
component is in 0..1. Sample any age in any order. Scene/camera transforms and
ambient rotation stay with the caller. Shader color is premultiplied linear HDR.

## Tuning and research

Set `PSYCHOPOMP_SHADER_DIR=crates/psychopomp-render/src/render` to reload shader
sources, including the `effects/` files. Keep short canonical studies under
ignored `output/`; keep the experiment method and decisions in `perf/`.

`python3 scripts/bench-stage-burst.py <label>` records deterministic pixel
continuity, local rim response, and the known volume-boundary regression.
See `perf/stage-burst-quality.md` for the scope of those metrics. Review normal
speed clips, contact strips, late cooling, and rewind before accepting tuning.

Water is a future effect family. Add it with a concrete scene and a small
study first, composing the noise, curves, motion, and optics that already fit.
Extract a shared solver or new crate when those uses demonstrate its
Interface. Preserve independent timing, spatial coordinates, seed identity, and
deterministic sampling across every composition.

## Lightning

Real lightning is not a line that slides. A dim stepped leader feels its way
out, the return stroke flashes the whole channel white for a frame or two,
further strokes re-light it a few frames apart, and the ionized channel cools.
`effects::lightning::Discharge` owns that on one clock, a bolt's `age` in
seconds: a 75 ms leader that advances in whole 60 fps steps, then `strikes`
(1..8, default 3) return strokes at seeded 45..110 ms intervals. The stroke
index is a step function of age, never a blend. Each stroke holds white-hot for
14 ms, decays with a 30 ms time constant, and leaves a 65 ms tone afterglow;
both envelopes reach exact zero 0.6 s after the stroke. The first stroke is the
brightest; re-strikes carry 55..95 % of it.

Paths are seeded midpoint displacement (`jag`): perpendicular offsets scaled by
each subdivided segment's length, so the jaggedness is self-similar and
independent of screen size; per-level roughness keeps broad bends gentle and
fine kinks sharp, and deviates are pushed toward their extremes so kinks read
as turns. A `Roll` carries two seeds: the discharge's `shape` fixes the coarse
channel (its first three levels, mostly), while each stroke's `strike` re-rolls
the detail and every branch, so re-strikes flicker around one recognizable
channel. `writhe` slides the coarse channel continuously for sustained arcs.
Branches fork mostly early, lean toward the receiver, shorten along the
channel, and sometimes fork again; the leader reveals them in fork order.

A Stage `bolt` element strikes between two positioned elements, shields, or
world points, at the outline crossings of the straight line between them
(`Shape::boundary_toward`). Its channels are `age` (-1 idle), `seed` (an
integer; `zap` rolls a fresh one each strike, mixed with the element ID),
`opacity`, and `hum` (a sustained arc's intensity, 0..1.5). The renderer draws
each glowing stroke as primitive kind 10, a polyline whose points carry energy
and whose optics come from `lightning.wgsl`: pure emission, never occlusion, so
bloom turns the core into the tone-colored halo. Each stroke flashes both
contacts and throws seeded ballistic sparks from the receiver along its surface
normal (gravity 1300 px/s², drag, cooling from white to the tone). Lights go
through the Stage's existing local-light path: the leader's tip, both contacts
as rim reflections, and a flood into the receiver, all strobing with the
strokes. Struck orbs take `surface::impact` from the contact (re-strikes more
softly); struck shields ripple. `zap` runs the clock past the last stroke for
`shield::RIPPLE` so those reactions finish.

`crackle` puts short arcs on a charged card's or orb's outline (`charge`
channel, 0..1.5): nine seeded lanes join as the charge rises, each crawling
along the perimeter over a 0.22..0.57 s life and re-striking with fresh detail
every 30 Hz slot, with some slots dark so it strobes. Arcs follow the outline,
bow off it, and sometimes throw a twig; the three brightest light their own
rim. `hum` re-strikes at 24 Hz while the coarse channel writhes, with
occasional dropouts. Both are pure functions of scene time and seeds.

## Dissolve, scan, and shield

A card's `dissolve` is a clock in seconds (≤ 0 intact; gone at `BURN`, 1.1 s;
the last ash cold at `DURATION`, 2.05 s). The burn front sweeps a field of
seeded integer-hashed value noise (bit-identical between `math::random` and
`dissolve.wgsl`) leaning from the top left corner. Any primitive can carry the
mask (`Prim::mask`, `mask_shape`, `mask_color`; zero is none), so fills, rims,
and glyphs burn together: pixels just ahead of the front scorch, a thin rim
behind it glows from the tone to white-hot, and what it passed is gone. 160
ash flakes leave exactly where the rim crosses their home (`passes`), drift up
under buoyancy and drag, and cool. `materialize` plays the clock backwards,
rushing through the empty end and settling at a third of its average speed, so
the ash flies home and the rim recedes. A clock rather than a 0..1 progress
keeps the ash moving in real time after the card is gone.

A card's `scan` (0..1, invisible at both ends) sweeps a line down it as
primitive kind 12: the line overhangs the sides a little, a wake fades behind
it inside the panel, and the rim lights where the line crosses it.

A `shield` element wraps a positioned element in a bubble (primitive kind 11):
hexagonal cells laid out stereographically so they crowd toward the limb, a
Fresnel rim, and a faint flowing shimmer. `up` (default 1) switches cells on
in seeded order, each flashing as it lights. Packets crossing into it on their
way to what it surrounds ripple it softly; bolts that strike it ripple it with
each stroke and flare its contact light (`shield::flare`). Up to four
contacts, most recent first, travel over the sphere as `surface::impact`.

`camera.shake` is trauma in 0..1; `StageActor::jolt` sets it with a `hit` and adds
the spring-loaded `camera.kick-x|y` shove and a `camera.punch` zoom.
`camera.quake` is sustained trauma a scene ramps itself to build tension; it adds
to `shake`, and the sum may overdrive to 2 (four times a full hit's rumble).
`post.zoom` (0..0.5) averages 16 taps along each pixel's ray to the frame center,
a radial streak for lunges and whip pans; `post.flash` (0..1) washes the developed
frame toward white before the vignette, for impacts that blind or to hide a cut. The renderer
moves the camera by kick plus rumble in every shutter sample, so the shake
motion-blurs; roll and punch transform the developed frame in the composite, and
the punch also covers corners a roll would expose. Plan overlays (headers,
captions) stay still, like a HUD. Time per-element reactions with
`combustion::shock_arrival` so they land as the pressure front passes.

Camera shots and impacts add rather than fight. The renderer starts from the
authored pose (`camera.x|y|z|yaw|pitch|zoom|pivot`), blends in any follow
(`camera.track.<id>`), then adds handheld sway (`camera.handheld`), the kick,
and the rumble, all per shutter sample. An authored `camera.roll` turns each
sample as it is exposed (so it blurs) and crops to cover the corners; the shake
roll and punch still transform the developed frame. `CameraRig::whip` pairs a
short minimum-jerk move, which the shutter streaks, with a `post.zoom` swell
through its middle.
