# Daemon diagram playback performance

**Historical CPU pass.** Kit subsequently rejected the surrounding card UI.
The bare flat/isometric GPU replacement and shared browser proof are recorded in
`perf/diagram-gpu.md`. The CPU backdrop cache below is no longer in this recipe;
its measured results and discarded experiments remain useful historical evidence.

## Target and fixed workload

Make the native `scenes/opencode-architecture` diagram responsive at 60 Hz without
changing its Scene Plan, 1920×1080 source, one-sample native profile, fractional
sampling, blur, theme, or export shutter sampling.

Primary metric: native median-round p95 submission interval, measured by the
existing `--benchmark` (one warmup second + nine interruption rounds). These are
submission measurements, not scanout. Secondary metrics: frames submitted per
second, worker render time, and upload/preparation time.

The GPU-initialized but CPU-rendered fixed-clock test isolates recipe cost from
the window: one warmup + seven measured runs of 120 samples at authored 60 Hz,
Next / Last / Previous / First every 15 samples. It includes Playback sampling,
the worker's visual key, and full recipe painting, but not presentation/upload.
Navigation is timed separately. PNG writing is outside the measured interval.
The same test saves 120 interrupted native samples and 75 out-of-order authored
samples across all themes and a return to Original for exact before/after checks.

```sh
cargo build --release
target/release/psychopomp plan present target/opencode-architecture/daemon-merge.json --theme original --benchmark > output/daemon-speed/native.json
PSYCHOPOMP_DIAGRAM_PERF="$PWD/output/daemon-speed/candidate" cargo test -p psychopomp-render --release diagram_sampling_benchmark -- --ignored --nocapture --test-threads=1
```

Do not measure concurrently with builds or other rendering benchmarks. Artifacts
are ignored under `output/daemon-speed/`, with the original binary frozen under
`target/daemon-speed/`. The existing user viewer is not used as a benchmark driver.

## Baseline

Apple M2 Max / Metal, release, 2560×1440 native window, 60 Hz FIFO:

- Two repeats: **105.79 / 105.99 ms** median-round p95 submission interval.
- **16–17 submissions/sec** in every measured round.
- Median worker render: **44.50 / 45.26 ms**; p95 **69.28 / 69.27 ms**.
- Median upload/preparation: **1.085 / 1.053 ms**.
- Raw: `native-before-{1,2}.json`. This reproduces the reported slowness; it is
  not simply the intentionally authored spring duration or slow-motion setting.

## Ranked hypotheses

1. Repainting the immutable dotted stage/title dominates each frame. Cache one
   theme's exact backdrop and clone it before painting animated content.
2. Small flat boxes pay for general perspective, sampling and material composition.
   Measure their remaining share before adding a specialization.
3. Dynamic path/text work or navigation dominates the remainder. Measure before
   modifying trajectories, property lookup, or pixel recipes.

Each experiment must demonstrate a durable win and exact candidate/control PNGs.
No broad compositor rewrite or new renderer backend is assumed necessary.

## 1. Immutable backdrop — kept

The fixed-clock median fell from **45.892 ms** (MAD 0.278) to **21.330 ms**
(MAD 0.285). All **195/195 PNGs are byte-identical**. `DiagramGlyphs` retains one
current-theme/current-size backdrop with its existing tinted glyphs. It clones
that image before painting nodes, wires, indicators and captions. This adds
8,294,400 bytes (7.91 MiB) per prepared diagram, not a multi-frame animation cache.
Cold theme switches still paint it once; settled scene behavior is unchanged.

This is not yet enough for the 16.67 ms frame budget. The next measurement profiles
the remaining general card sampling rather than changing authored motion timing.

## 2. Constant-neighborhood sampling — kept

A four-second CPU sample after backdrop caching attributed about 83% of render
stack samples to `FrameUi::card_source`; about 57% of total render samples landed
inside `RgbaSource::sample` under the blur kernel. These are statistical stack
samples, not isolated function wall times (`profile.txt`).

The candidate skips bilinear premultiply/interpolate/unpremultiply only when all
four source texels are identical. Fully transparent neighborhoods still return
transparent black; mixed neighborhoods, edge clamps, fractional coordinates,
blur taps, and perspective transforms retain the original filter. A differential
unit test compares over 230,000 constant and mixed/strided samples against the
original filter, including every byte value and near-integer positions.

Fixed-clock median **15.295 ms**, MAD **0.077**, p95 **30.930 ms**; **195/195 exact
PNGs**. Native follow-up: **47–48 submissions/sec**, median-round p95 **32.28 ms**,
median worker render **15.06 ms**. This is a clear improvement, but blur-heavy
reversals still miss the 60 Hz budget. Navigation is only about 0.11 ms/action and
is not the demonstrated bottleneck; no property/Timeline cache was introduced.

## 3. Uniform blur taps — discarded

Sample the same nine taps, in the same order, then skip weighted accumulation
only if their RGBA values are all identical. Mixed neighborhoods retain the
original weight order, precision and rounding. The reference test also covers
the nine-tap filter and its sharp/blur threshold.

The CPU median improved slightly to **14.483 ms** (MAD 0.413), with p95 28.433 ms
and **195/195 exact PNGs**. Native throughput increased to 49–50 submissions/sec,
but median-round p95 intervals were **35.17 / 35.16 ms** versus the sampler's
initial 32.28 ms. Later retained-build repeats below also reached 37–38 ms, so
this is not strong evidence of a causal regression. It did not establish a
repeatable primary pacing win; the extra nine-tap accumulation logic was removed.
The original blur loop and weights remain intact. Reports: `uniform-blur/`,
`native-blur-{1,2}.json`.

An additional inline hint on `RgbaSource::sample` measured **14.400 ms** (MAD
0.157), p95 28.996 ms: noise relative to the blur candidate, also removed. No
inline hint, reduced filter kernel, or pixel-opacity cutoff remains.

## Final retained build

Two matched native repeats (`native-final-{1,2}.json`):

| Metric | Before | Retained build |
| --- | --- | --- |
| Median-round p95 submission interval | 105.79 / 105.99 ms | 38.24 / 37.45 ms |
| Submissions per measured second | 16–17 | 44–49 (mostly 47–48) |
| Median worker render | 44.50 / 45.26 ms | 15.01 / 14.78 ms |

The final fixed-clock repeat (`final/timing.json`) measured **14.836 ms** median,
MAD **0.074**, p95 **30.345 ms**, versus the original 45.892 / 70.726 ms. All
**195/195 final PNGs match the frozen original byte-for-byte**. The Scene Plan is
also byte-identical. The result is roughly three times the original throughput,
not a locked-60-fps result or a scanout measurement.

Stop this scoped pass here: the expensive rapid blur-heavy reversals still miss
16.67 ms, and the smaller trials did not establish a durable pacing improvement.
Another pass should target the remaining card/filter pixel work with a fresh
profile and these same gates, rather than accumulating speculative caches or
retiming the reference. No browser or native presentation scheduler was changed.

## Verification

- Workspace tests, formatting, strict workspace Clippy and `git diff --check` pass.
- All **38 ignored GPU/artifact tests** pass, including the fixed-clock benchmark,
  cold/warm backdrop parity, theme round-trips, native/export onset correspondence,
  retained client pixels, cancelled waits and interrupted position/velocity/pixels.
- Final **195/195 PNGs** match the frozen original; no scene/choreography edits.
- Both 72-frame shutter-sampled exports (entrance and merge) and the 132-frame
  interrupted native-path movie decode to exactly the same frames as before.
  Per-frame FFmpeg hashes are under `verification/`; full-scale output and a
  stepped merge strip were inspected.
- A perspective/blurred hero-editor frame at 0.23 s also remains byte-identical
  to the frozen binary, exercising another consumer of the shared card sampler.
- No GPU, font or FFmpeg constraint blocked these checks. Live native scanout is
  not captured; the native benchmark measures submissions, not display scanout.
