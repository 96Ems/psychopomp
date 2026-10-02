# Browser grid autoresearch

Goal: lower the cost of the shared chess renderer in the browser without reducing
resolution, AA, temporal quality, motion continuity, or native/browser sharing.

## Benchmark

Prepare without touching the live demo:

```sh
PSYCHOPOMP_WEB_ASSETS="$PWD/target/browser-perf-site" bash experiments/browser-grid-prototype/run.sh --build-only
```

From `experiments/browser-grid-prototype`, run `bun install`, then
`bun run bench <label>`. `BENCH_ASSETS=/absolute/path` selects
a frozen asset build. Uses installed Chrome (`CHROME` can override the path) in
its own headless process/profile, never Kit's live Browser Control tab.

Primary: median **GPU-completed wall milliseconds per frame**, one warmup plus
seven measured runs. Each run samples 240 1080p frames at authored 60 fps, one
temporal sample, 4× spatial AA, a navigation command every 15 frames, and fences
the GPU queue every 12 frames. This is a bounded-throughput benchmark, **not
display FPS or isolated GPU timestamp time**. CPU sample/submission and navigation
compilation are secondary metrics; median absolute deviation records noise.

The benchmark bypasses demo DOM diagnostics, holds the workload fixed, rejects
software adapters, and saves six authored plus sixteen interrupted-navigation PNGs
outside measurement. Every candidate must match baseline pixels. Do not time a
build concurrently. The shared host/GPU can still be affected by other processes.
Results live under ignored `output/browser-perf/<label>/`.

## Scope and hypotheses

- Shared `plan_runtime/grid.rs`: repeated string/property allocation in sampling.
- Shared grid atlas: RGBA storage although only glyph alpha is sampled.
- Shared grid passes: bandwidth/attachment work beyond visible geometry.
- Browser host/compiler settings: investigate only against the same workload.

No lower-resolution or lower-AA shortcut. No separate browser physics/renderer
fork. Keep clear wins; discard noise or changed pixels. Record each experiment
and its countercheck below before stacking another change.

## Results

Baseline runs: 0.4154 ms/frame (MAD 0.0188), repeat 0.3754 (MAD 0.0067).
CPU sample/submit 0.1496–0.1571 ms/frame; navigation 0.4375–0.4500 ms/action.
Adapter reports Apple / metal-3. Existing one-off rAF results are not this baseline.
Frozen baseline assets: `target/browser-perf-baseline`; live assets on port 5201
remain untouched throughout experiments.

1. Hypothesis: preparing stable per-item PropertyIds once eliminates hundreds of
   formatting/allocation operations on every sample. Cache names, not track indices
   or values, so immutable Timeline revisions remain correct. **Discarded.**
   Same-process, alternating-order pairs showed CPU cost reduced ~47–51%, but
   completed-frame ratios were 1.012, 0.976, 1.013: no durable primary-metric gain.
   Do not keep the extra cache just because the micro-metric looks good.

Benchmark correction: fresh Brave profiles/origins changed a few hundred canvas
readback components by ±1 even for unchanged baseline builds (482 at t=0 in one
repeat). Compare candidate/control in the **same origin/session**, which produced
22 exactly matching images for the property-ID trial. This is consistent with
canvas privacy noise, not a demonstrated rendering regression. Paired variants
also alternate order in one browser process to reduce host-load drift. The two
devices increase resident GPU memory; paired results are not single-embed memory
measurements. `BENCH_COMPARE=<frozen-assets>` enables this mode.

2. Bound the stroke composite to sampled geometry and clear untouched background.
   **Discarded:** paired completed ratios 0.991 and 1.013, inside noise. All 27
   same-session pixel cases matched exactly, but extra clipping logic bought no
   durable speed improvement on this GPU. Native/source files restored exactly.

3. Browser delivery currently draws into a full RGBA texture, then samples it in
   another full-screen pass into the canvas. Hypothesis: configure a matching
   RGBA canvas view and let the unchanged shared grid passes render directly into
   it. Removes one full-size texture and one pass without changing the renderer.
   **Kept:** paired completed ratios 0.969 and 0.970 (~3% less work time), with
   27/27 byte-identical same-session frames. More importantly, a deterministic
   8,294,396-byte reduction in requested textures (7.91 MiB; the constructor has
   a temporary 1×1 placeholder) and fewer host code/pipeline/bind-group resources.
   This is a small throughput gain, not a claim of noticeably higher display FPS.

4. Shared atlas stores four 8-bit channels, but `label_ink` reads only alpha.
   Hypothesis: store that unchanged 8-bit coverage in R8, sample `.r`, and retain
   all layout/filtering/theme behavior. Expect 75% lower atlas allocation/upload,
   no quality loss and possibly less texture bandwidth. Measurement pending.
   **Kept for simpler data/storage, not speed:** ratios 0.998 and 1.010 versus
   direct-canvas alone are noise. Atlas requests fell from 11,010,048 to 2,752,512
   bytes (75%) with 27/27 exact pixel matches. No precision or filtering reduction.

## Final and stop condition

Requested explicit textures: **176,898,048 → 160,346,116 bytes**, saving
**16,551,932 bytes / 15.79 MiB / 9.36%**. This includes the label atlas and temporary
1×1 view, excludes canvas swapchain, and is allocation-descriptor accounting—not
physical VRAM, driver compression, or a GPU-process memory measurement.

Combined paired repeats: completed ratios **0.9445** and **0.9883**; absolute
candidate medians 0.3904 and 0.3971 ms/frame. The small overall timing gain varies;
do not claim a dramatic FPS improvement. Resolution, AA, temporal samples, scenes,
theme, and motion are unchanged. The two kept changes remove redundant work/data;
both speculative caches/scissors were removed. Bigger memory savings need an
attachment-size/lifetime design, or an explicit quality budget, not micro-tuning.

Stop here. Final verification passed:

- Workspace tests, formatting, strict workspace Clippy, and all **35 GPU tests**.
- Strict native/WASM prototype Clippy; release native frame and WASM builds.
- Native 17-second grid frame byte-identical to its pre-change production render.
- **27/27 exact candidate/control images** in each final paired browser run.
- Actual component/rAF smoke: navigation, pause, exact frame stepping, speed,
  theme clock stability, reduced motion, scene change and teardown, no browser
  errors. Captures and a decoded/stepped video live in `output/browser-perf/smoke`.
  An initial smoke fixture omitted the fallback PNG; supplying that generated
  asset fixed the 404 without suppressing errors or changing application behavior.

No GPU/font/FFmpeg constraint blocked this verification. No test drove the user's
active Browser Control page. Port 5201 now serves the verified final WASM bytes;
the user can reload when ready. The private smoke server on 5202 was stopped.
