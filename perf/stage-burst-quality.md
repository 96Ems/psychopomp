# Stage Burst Quality Loop

## Goal

Improve the flagship's visual continuity, then deliver the full narrated film.
The primary measurable defect is the instantaneous switch from intact orb to
Burst at age zero (and back at the end of rewind). This is a continuity proxy,
not a numerical claim about artistic quality.

## Benchmark

`cargo build --release` then
`python3 scripts/bench-stage-burst.py baseline` (Pillow and numpy required).

- Frozen scene time, camera, and 1080p render; vary only Burst age −1 → 0.
- Mean absolute RGB8 error in the orb region (lower is better, ideal zero).
- One warmup, seven measured pairs; report median and MAD. Identical values are
  expected because rendering is deterministic.
- Retain PNGs and machine-readable results under `output/burst-research/`.
- Guardrails: silhouette, emission, smoke, attachments, text hierarchy, and
  normal-speed/40 ms motion studies. A better score alone cannot approve the look.

## Hypothesis 1

The dedicated Burst particle painter switches particle opacity, size, shading,
and occlusion immediately at age zero. Preserve the intact material at the
handoff, then let compression and ignition change it continuously.

## Results

| Experiment | Entry MAE RGB8 (median / MAD) | Changed pixels | Decision |
|---|---:|---:|---|
| Baseline | 9.354957 / 0 | 73.14% | Reference |
| Continuous shell-to-ignition handoff | 0 / 0 | 0% | Keep, subject to motion review |

Seven measured pairs per version. The intact shell now compresses with its own
material and body occlusion; a 55 ms ignition envelope hands presence to the hot
particles. The volume retains its existing ignition timing. GPU regression checks
cover both exact age zero and a positive epsilon, so an isolated special-case
frame cannot satisfy the continuity contract.

## Hypothesis 2

The fire emits no scene lights, so nearby card surfaces remain disconnected from
the combustion. Cast one age-driven local reflection plus a restrained pool
through the existing light path. Measure the client rim facing the fire against
its far rim at age 0.45 seconds: desired near-rim red lift 3–20 RGB8 levels,
far-rim lift below 1. This bounded target catches both invisible light and a
whole-card wash; inspect the matching frame and motion study as a visual gate.
The fixture has no packets, captions, or animated camera. Sampling excludes the
socket and text. Entry MAE must stay zero.

Result: seven runs, near-rim red lift **0 → 6.166667**, far-rim lift **0 → 0**,
entry MAE still zero. Kept the existing rim reflection path, with no fill pool.
Full-scale frame review confirms a warm facing rim and an unchanged dark body.

## Hypothesis 3

Full-scale inspection exposed straight cuts in the fire silhouette: the density
field can remain positive outside the shader's early-rejection bounds. Give the
volume compact support that reaches zero before every rejection plane, preserving
the inner turbulence. Measure the 99th percentile horizontal RGB step in the
left cutoff region of the fixed hot frame (x750..790, y429..486). This is a
defect-specific edge metric, not a general preference for blurred images. Keep
only if the cutoff disappears while inner flame detail remains readable.

Result: seven runs, edge-step p99 **33 → 17 RGB8** (48.5% reduction). Full-scale
before/after review shows the straight cuts gone with inner turbulence retained.
Entry MAE and rim-light locality are unchanged. The volume quad also grows from
4 to 4.4 radii to cover the bounded field's late upward drift. **Keep.**

Stop after these three concrete defect fixes. Further brightness/detail tuning
would be an aesthetic choice without a demonstrated defect; keep the restrained
scene and inspect the complete narrated film next.

## Effects extraction verification

The extracted CPU and binding-free WGSL Modules preserved all three benchmark
metrics (`effects-module`, seven pairs). Decoded full-resolution PNGs at 3.8,
18.67, 19.08, 21.5, and 29.9 seconds had a maximum channel difference of **zero**
against the pre-extraction build. This covers the intact shell, ignition, fire,
smoke, and rewind; it is a finite parity check, not proof for every input.

## Follow-up: contact, code, and camera

The next review identified three separate issues:

- Request arrival read as a vague center pulse. `effects::surface` now samples a
  local dimple and an emissive wave from the packet's visible-shell crossing.
  The encoded 10.3–11.7 s study, inspected at 40 ms intervals, shows the wave
  starting on the left and traveling across the orb before cooling.
- Adjacent diff backgrounds double-blended fractional row edges. Weighted
  interval union removes the seam: at the 58 s hold, the blank stripe
  x263..268/y335..591 changed from **6 to 0 RGB8** green variation and maximum
  adjacent-channel step. Fractional adjacency and unequal-opacity overlap have
  CPU regression tests.
- Camera glides felt stiff. The flagship now uses critically damped camera
  springs. The encoded 45–49.9 s study was inspected at 200 ms intervals for
  framing and the client-card zoom handoff; subjective playback approval remains
  with the viewer.

The encoded code study (50.3–54.9 s, inspected at 100 ms intervals) also exposed
the opportunity to retain the declaration and version-check expression through
the first edit. The two lines retain identity while exchanging positions;
only their edited inline parts change. `plan steps` on the extracted code plan
reports no warnings at two-second holds, and a regression test checks that the
common parts are absent from changed-part IDs. This film review does not establish
interactive reverse/skipped-step navigation behavior.

Workspace tests, formatting, strict Clippy, and both ignored Stage GPU tests pass.
Local artifacts and validation logs are under ignored `output/`.
