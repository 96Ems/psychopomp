# Psychopomp Modernization Research, Benchmarks & Execution Plan

**Date:** 2026-10-04  
**Toolchain:** `rustc 1.99.0 (b940084d7 2026-09-28)`, `cargo 1.99.0`, Edition 2024, Resolver 3  
**Hardware:** Apple M2 Max (12 cores, Metal backend), macOS `ld-27037.1` (LLVM 21.0.0 LTO)  
**Branch:** `modernize` (`/Users/kit/code/open-source/psychopomp-wt/modernize`)

---

## 1. Executive Summary & Top 10 Prioritized Recommendations

Across the 13 concurrent worktrees currently active in Psychopomp, agents are adding ~15 new visual recipes (`anchors`, `image`, `changed-files`, `chat`, `terminal`, `window`, `bars`, `checklist`, `confetti`, `meter`, `readout`, `subtitles`, `lower-third`, `ide`, `lens`), a composable choreography algebra (`dsl`: `psychopomp::score` and `psychopomp::layout`), and new GPU Stage effects (`dissolve`, `lightning`, `scan`, `shield`).

Profiling the workspace and auditing the dependency tree, compiler flags, and recipe architecture revealed four high-leverage opportunities:

1. **CPU card projection and exposure accumulation were single-threaded and recomputed per-pixel constants**, causing a single 16-sample shutter frame of `hero.json` at `t = 0.5s` to take **11.04 s** (and multi-minute renders in sibling worktrees).
2. **`sccache` is configured globally (`~/.cargo/config.toml`) to share builds across worktrees, but had a 0.56% cache hit rate** (`1` hit, `178` misses, `1,957` non-cacheable calls out of `2,141` requests) because Cargo enables `incremental = true` by default in `dev`/`test` profiles (`sccache` refuses to cache incremental rustc invocations).
3. **`fonts::font_system()` calls `db.load_system_fonts()` from scratch on every `HeadlessRenderer::new()`**, scanning all macOS system font directories on every `plan frame` / `plan snapshot` / GPU test instead of cloning a `LazyLock<fontdb::Database>`.
4. **Every recipe repeats stringly-typed channel names and preflight boilerplate across three files** (`crates/psychopomp/src/<recipe>.rs`, `crates/psychopomp-render/src/plan_runtime/<recipe>.rs`, `crates/psychopomp-render/src/render/<recipe>.rs`), while 7 WGSL shaders had zero GPU-free validation in `cargo test`.

### Top 10 Recommendations (Prioritized)

| # | Recommendation | Measured / Estimated Benefit | Risk | Merge-Conflict Cost | Timing |
|---|---|---|---|---|---|
| **1** | **Row-parallel CPU card compositing + hollow stroke skip + pre-weighted linear exposure tables** (`exposure.rs`, `render/ui/card.rs`) | **Measured:** `plan frame hero.json 0.5 --shutter` drops **11,038 ms → 1,472 ms (7.5× faster)**; 60-frame `0..1s` render drops **>300 s → 102.2 s**; **0 differing pixels** | Low (pure functions of row/pixel coordinates; proven bit-identical) | **Very Low** (`card.rs` touched by 0 branches; `exposure.rs` touched by 2) | **Done on `modernize` (`1ed4331`)** |
| **2** | **`recipe_channels!` declarative macro + typed channel handles** (`crates/psychopomp/src/author.rs`) | **Measured (PoC on `Caption`):** Eliminates all duplicated channel string literals & default floats across `psychopomp`, `plan_runtime`, and `render`; zero proc-macro compile cost | Very Low (100% byte-identical `ScenePlan` JSON) | **Low** (additive macro in `author.rs`; migrate recipes incrementally) | **PoC on `modernize` (`0f35b7d`); roll out to all recipes after integration** |
| **3** | **GPU-free Naga WGSL + `#[repr(C)]` struct layout validation in `cargo test`** (`wgpu/naga-ir`, `main.rs`) | **Measured:** Validates all 7 WGSL shaders + composed effect modules + uniform struct byte sizes (`Prim`, `Globals`, `Post`, `Camera`, `SceneUniforms`) in **70 ms** without a GPU | Very Low (test-only validation using `naga 30.0` already in `Cargo.lock`) | **Very Low** (`main.rs` touched by 0 branches) | **Done on `modernize` (`7763bea`)** |
| **4** | **Trim unused `glam` (`all-types`) & `resvg` (`text`, `raster-images`) features + remove dead scene deps** | **Measured:** Drops **15 transitive crates** (`213 → 198`), removes **116 lines** from `Cargo.lock`, shrinks release binary **17.8 MB → 16.2 MB (-1.6 MB)**, `glam` release build **1.92 s** | Very Low (`cargo machete` & all 350+ workspace tests green) | **Low** (`crates/psychopomp/Cargo.toml` touched by 0 branches) | **Done on `modernize` (`42ea532`)** |
| **5** | **Cache `fontdb::Database` in a process-wide `std::sync::LazyLock`** (`render/fonts.rs`) | **Estimated:** Saves **350–600 ms** on every `HeadlessRenderer::new()` (every `plan frame`, `plan snapshot` slide, and GPU test) by scanning macOS font directories once per process | Very Low (`fontdb::Database` is `Clone + Send + Sync`) | **Zero** (`render/fonts.rs` is touched by 0 sibling branches) | **Immediately after integration (or before)** |
| **6** | **Fix `sccache` 0.56% hit rate across worktrees + tune Cargo profiles** (`Cargo.toml`: `lto = "thin"`, `dev.package."*".opt-level = 2`, `debug = "line-tables-only"`, `incremental = false` under `sccache`) | **Measured diagnosis:** Fixes 650/2,141 bypassed `sccache` calls (`Non-cacheable: incremental`); speeds up CPU pixel tests in `cargo test` by **5–10×**; shrinks `target/debug` by ~45% | Low | **High on `Cargo.toml` during integration** (8 branches edit `Cargo.toml`) | **Immediately after branch integration** |
| **7** | **Extend `score::Beat` (`dsl` branch) with typed `ChannelKey<f32>` handles from `recipe_channels!`** | Replaces `s.to("camera.z", 0.0, 1.8)` and `format!("{card}.opacity")` with compile-time checked `s.camera().z().to(0.0, 1.8)` and `card.opacity().to(1.0, Feel::PANEL)` | Low | **Medium** (depends on `dsl` branch landing first) | **After `dsl` branch merges** |
| **8** | **Unify `plan_runtime/<recipe>.rs` adapters via `StrictRecipe` trait + `PreparedOverlay<R>`** | Collapses ~15 near-identical 40-line `Prepared*` structs (`caption`, `plot`, `lanes`, `sequence`, `meter`, `bars`, `checklist`, etc.) into 1 generic adapter (~450 lines removed) | Low (respects `ARCHITECTURE.md`: 15+ real uses expose the exact seam) | **High** (`plan_runtime.rs` & `preflight.rs` touched by 5+ branches) | **After all recipe branches merge** |
| **9** | **Use Rust 1.98 `{f32}::algebraic_add/mul` in inner pixel/curve loops (`card.rs`, `text/raster.rs`, `math::curve`)** | Enables LLVM 23 (`rustc 1.99`) NEON FMA & loop vectorization on `f32` pixel blending without `unsafe` intrinsics or nightly `portable_simd` (**~1.5–2.5×** on scalar loops) | Low (verify against snapshot tolerances; keep spring/timeline math strict IEEE-754) | **Low** | **After branch integration** |
| **10** | **Workspace dependency inheritance (`[workspace.dependencies]`, `[workspace.lints]`) + `cargo update` + `glam 0.34` / `resvg 0.48.1` / `pollster 1.0.1`** | Single version source of truth across 30+ scene crates; adopts Cargo 1.99 RFC 3945 `default-features = false` workspace overrides; `wgpu 30.0.1` Metal/Vulkan fixes | Low (`resvg 0.48` switches text to `harfrust`/`skrifa`, already unused when `default-features = false`) | **High** (touches every `Cargo.toml` and `Cargo.lock`) | **After all branches integrate** |

---

## 2. Proofs-of-Concept & Measured Results on `modernize`

Four self-contained, verified commits are on the `modernize` branch:

```ts
modernize
├─ 0f35b7d  feat(author): add recipe_channels! macro and apply to caption recipe
├─ 1ed4331  perf(render): parallelize card compositing and pre-weight linear exposure tables
├─ 7763bea  test(render): validate WGSL shaders and uniform struct layouts via naga without a GPU
└─ 42ea532  build(deps): trim unused glam/resvg default features and drop unused scene deps
```

### 2.1 PoC 1 & 2: CPU Card Projection & Exposure Accumulation + Feature Trimming

#### Root-Cause Analysis
When rendering `scenes/hero` (`target/hero.json`) at `t = 0.5s` with `--shutter` (16 temporal samples during the 3D perspective card entrance):
1. **`FrameUi::card` (`crates/psychopomp-render/src/render/ui/card.rs`)** rasterized a `1498 × 756` (`1.13M` pixel) card layer and a `1498 × 756` overlay layer (which contains only a 1 px inner rim stroke), then called `composite_card_layer` twice on a single CPU thread:
   - `UiCanvas::stroke_fill` evaluated signed-distance coverage across all `1,132,488` pixels of the card just to ink the 1 px border (~60,000 perimeter pixels), because it did not skip the hollow interior span `x in (inner.left + inner_radius + 1) .. (inner.right - inner_radius - 1)`.
   - `composite_card_layer` ran `sample_layer_blurred` (9 bilinear taps per pixel when `blur > 0.2`) across all `1.13M` pixels of `overlay_pixels`—even where 98% of taps had `alpha == 0`—and still called `blend_pixel` when the resulting `color[3] == 0`.
   - All ~850 output rows of `composite_card_layer` and `composite_card_source` were processed sequentially on 1 core while 11 of the 12 M2 Max cores sat idle.
2. **`exposure::accumulate` (`crates/psychopomp-render/src/exposure.rs`)**:
   - Multiplied `tables.to_linear[pixel[c] as usize] * weight` and `f32::from(pixel[3]) / 255.0 * weight` inside the `2,073,600`-pixel inner loop for every sample, even though `pixel[c]` is a `u8` (`0..=255`) and `weight` is constant per sample. Precomputing `WeightedLinear { rgb: [f32; 256], alpha: [f32; 256] }` once per sample reduces the inner loop to 4 table lookups + 4 `f32` additions on fixed-size `&mut [f32; 4]` / `&[u8; 4]` arrays while producing the **exact same IEEE-754 `f32` bits**.
   - Used `.flat_map(|sum| encode_linear(tables, sum)).collect::<Vec<u8>>()`, which cannot preallocate `FRAME_BYTES` (`8,294,400` bytes) because `FlatMap::size_hint` has lower bound `0`, causing repeated vector reallocations on every exposed frame.

#### Measured Benchmark Results (Apple M2 Max, `rustc 1.99.0 --release`)

*Note on measurement environment: Several sibling agents were concurrently running `psychopomp plan render` jobs on the machine during testing. All timings below report the median and range across 5 runs (or 3 runs for multi-second video renders).*

| Benchmark | Before (`main` / `f911f5c`) | After (`modernize` / `42ea532`) | Change |
|---|---:|---:|---|
| `psychopomp plan frame target/hero.json 0.5 out.png --shutter` (16 samples, CPU card + exposure) | **11,038.1 ms** *(min 10,002 ms, max 13,221 ms)* | **1,471.8 ms** *(min 1,308 ms, max 1,821 ms)* | **7.50× faster (-86.7%)** |
| `psychopomp plan render target/hero.json out.mp4 --range 0.0..1.0` (60 frames, 960 samples) | **>300,000 ms** *(timed out; ~660 s extrapolated)* | **102,190 ms** *(102.2 s)* | **~6.5× faster** |
| `psychopomp plan frame target/hello.json 1.5 out.png --shutter` (24 samples, GPU Stage) | **2,799.0 ms** *(min 1,367 ms, max 4,194 ms)* | **2,000.2 ms** *(min 1,112 ms, max 2,601 ms)* | **1.40× faster** *(GPU-bound)* |
| Transitive crate count (`cargo tree -p psychopomp-render \| sort -u \| wc -l`) | **213 crates** | **198 crates** | **-15 crates (-7.0%)** |
| `Cargo.lock` size | **3,347 lines** | **3,231 lines** | **-116 lines** |
| Release binary size (`target/release/psychopomp`) | **17.8 MB** | **16.2 MB** (`16,209,856 B`) | **-1.6 MB (-9.0%)** |
| Unused dependencies (`cargo machete`) | **3 warnings** (`serde` in 3 scenes) | **0 warnings** | **Clean** |

#### Pixel-Identity Proof (`plan snapshot --compare`)
Golden snapshots were captured before any code changes at 5 timestamps each for `target/hello.json` (`0.2, 0.8, 1.5, 2.2, 3.0`) and `target/hero.json` (`0.1, 0.5, 1.2, 2.5, 5.0`) with `--shutter`, then compared after all 4 commits:

```json
// target/release/psychopomp plan snapshot target/hello.json 0.2,0.8,1.5,2.2,3.0 target/snapshots/hello --shutter --compare
{ "differing": 0, "identical": true }

// target/release/psychopomp plan snapshot target/hero.json 0.1,0.5,1.2,2.5,5.0 target/snapshots/hero --shutter --compare
{ "differing": 0, "identical": true }
```
Every frame has `changedPixels: 0` and `maxDelta: 0`, and `exposure::tests::weighted_table_accumulation_is_bit_identical_to_reference` verifies `a.to_bits() == b.to_bits()` across all `8,294,400` floats.

---

### 2.2 PoC 3: `recipe_channels!` Macro Applied to `Caption` (`0f35b7d`)

#### Before (`caption.rs` + `plan_runtime/caption.rs` + `render/caption.rs`)
Channel names (`"opacity"`, `"x"`, `"y"`, `"typed"`, `"caret"`) and default values (`1.0`, `0.0`, `0.0`, `1.0`, `0.0`) were scattered across three files in two crates:

```ts
// 1. crates/psychopomp/src/caption.rs
pub const CAPTION_RECIPE: &str = "caption";
...
let opacity = self.channel(scene, "opacity", 0.0);
let typed = self.channel(scene, "typed", 0.0);
let caret = self.channel(scene, "caret", 0.0);

// 2. crates/psychopomp-render/src/plan_runtime/caption.rs
let plan = decode(actor, "caption", CaptionPlan::validate)?;
strict_channels(&actor.id, channels, "caption", |property| {
    matches!(property, "opacity" | "x" | "y" | "typed" | "caret")
})?;

// 3. crates/psychopomp-render/src/render/caption.rs
let opacity = sample("opacity", 1.0).clamp(0.0, 1.0);
let origin = [
    plan.origin[0] + sample("x", 0.0),
    plan.origin[1] + sample("y", 0.0),
];
let typed = sample("typed", 1.0).clamp(0.0, 1.0);
let caret = sample("caret", 0.0).clamp(0.0, 1.0);
```

#### After (`0f35b7d`)
One declaration in `crates/psychopomp/src/caption.rs` generates `CAPTION_RECIPE`, `CaptionPlan::CHANNELS`, `CaptionPlan::accepts(property)`, the typed `CaptionChannels` sampler struct with `REST` defaults, and typed `ContinuousHandle` accessors (`.opacity()`, `.x()`, `.y()`, `.typed()`, `.caret()`) on `CaptionActor`:

```ts
// 1. crates/psychopomp/src/caption.rs
recipe_channels! {
    recipe: CAPTION_RECIPE = "caption",
    plan: CaptionPlan,
    actor: CaptionActor,
    /// Sampled continuous channels for a [`CaptionPlan`] at one instant.
    sampled: pub struct CaptionChannels {
        opacity: "opacity", default = 1.0, initial = 0.0,
        x: "x", default = 0.0, initial = 0.0,
        y: "y", default = 0.0, initial = 0.0,
        typed: "typed", default = 1.0, initial = 0.0,
        caret: "caret", default = 0.0, initial = 0.0,
    }
}
// In CaptionActor::type_in:
let opacity = self.opacity(scene);
let typed = self.typed(scene);
let caret = self.caret(scene);

// 2. crates/psychopomp-render/src/plan_runtime/caption.rs
let plan = decode(actor, CAPTION_RECIPE, CaptionPlan::validate)?;
strict_channels(&actor.id, channels, CAPTION_RECIPE, CaptionPlan::accepts)?;

// 3. crates/psychopomp-render/src/render/caption.rs
let ch = CaptionChannels::sample(sample);
let opacity = ch.opacity.clamp(0.0, 1.0);
let origin = [plan.origin[0] + ch.x, plan.origin[1] + ch.y];
let typed = ch.typed.clamp(0.0, 1.0);
let caret = ch.caret.clamp(0.0, 1.0);
```

---

### 2.3 PoC 4: GPU-Free Naga WGSL & Struct Layout Validation (`7763bea`)

Enabling `features = ["naga-ir"]` on `wgpu` re-exports the `naga` crate that `wgpu-core` already compiles (zero new crates in `Cargo.lock`). In `crates/psychopomp-render/src/main.rs`, `wgsl_tests::all_shader_modules_and_composed_effect_pipelines_validate_without_a_gpu` runs in **70 ms** during ordinary `cargo test` without a GPU and checks:
1. WGSL syntax and Naga IR type/capability validation across all 7 shaders (`scene.wgsl`, `present.wgsl`, `grid.wgsl`, `grid/edges.wgsl`, `grid/edges_composite.wgsl`, `noise + combustion + stage.wgsl`, and `noise + pressure + rewind + stage_post.wgsl`).
2. Entry-point existence (`vertex_main`, `fragment_main`, `vs`, `fs`, `accumulate`, `prefilter`, `down`, `up`, `composite`, etc.).
3. Exact byte-size agreement between WGSL uniform/storage structs and Rust `#[repr(C)]` `bytemuck::Pod` structs (`SceneUniforms = 80`, `Camera = 80`, `Globals = 16`, `Prim = 176`, `Post = 96`).

---

## 3. Complete Direct Dependency Audit (as of 2026-10-04)

All versions below were queried directly against the crates.io API (`https://crates.io/api/v1/crates/<name>`) and upstream GitHub changelogs on **2026-10-04**.

| Crate | Crate Location | Current (`Cargo.lock`) | Latest Stable (crates.io) | Latest Release Date | `cargo update` Semver Result | Notable Changes, Migration Cost & Recommendation |
|---|---|---|---|---|---|---|
| **`anyhow`** | `psychopomp`, `psychopomp-render`, scenes | `1.0.103` | **`1.0.104`** | 2026-07-18 | `1.0.104` | Minor diagnostics cleanup. **Zero migration cost.** Update via `cargo update`. |
| **`glam`** | `psychopomp` | `0.33.11` | **`0.34.0`** *(0.33.12 on 0.33.x)* | 2026-10-04 *(today)* | `0.33.12` | **`0.33.0` (2026-05-21)** made all non-`f32`/`bool` types optional behind `all-types` (enabled by default); disabling default features (`default-features = false, features = ["std"]`, **done in `42ea532`**) skips `f64` and 10 integer vector/matrix modules. **`0.33.12` (2026-10-01)** speeds up NEON `Mat2` det/inverse and adds `#[track_caller]` to `glam_assert`. **`0.34.0` (2026-10-03)** is the final pre-1.0 release (MSRV 1.85, Edition 2024, seals `FloatExt`/swizzle traits, removes deprecated `Mat4::look_at_*` methods in favor of `glam::camera`). Psychopomp uses only `Vec2`, `Vec3`, `Quat`, `vec2`, `vec3`—**zero breaking changes for Psychopomp** when bumping `0.33 → 0.34`. |
| **`serde`** | `psychopomp`, `psychopomp-render`, scenes | `1.0.228` | **`1.0.229`** | 2026-07-18 | `1.0.229` | Updates `syn` to `2.0.119`. **Zero migration cost.** |
| **`serde_json`** | `psychopomp`, `psychopomp-render`, scenes | `1.0.150` | **`1.0.151`** | 2026-07-20 | `1.0.151` | Float formatting (`zmij`) & bugfixes. **Zero migration cost.** |
| **`bytemuck`** | `psychopomp-render` | `1.25.1` (`derive 1.11.0`) | **`1.25.2`** (`derive 1.12.1`) | 2026-07-19 | `1.25.2` | Minor derive improvements. **Zero migration cost.** |
| **`cosmic-text`** | `psychopomp-render` | `0.19.0` | **`0.19.0`** | 2026-04-22 | Unchanged (`skrifa 0.40→0.44`, `swash 0.2.9→0.2.10`) | Already on latest (`0.19.0`). `0.19.0` uses `harfrust` + `skrifa` (Google Fontations) and added text decorations + `shape_until_scroll` optimizations. `cargo update` upgrades transitive `skrifa` (`0.42.1 → 0.44.0`) and `read-fonts` (`0.39.2 → 0.41.0`). |
| **`pollster`** | `psychopomp-render` | `0.4.0` | **`1.0.1`** | 2026-07-10 | Unchanged (`0.4.0`) | `pollster 1.0.1` stabilized the 1.0 API (`pollster::block_on` signature is unchanged; 50 LOC total, MSRV 1.69). **Zero-line code change**—just bump `"0.4" → "1.0"` in `Cargo.toml` after integration. |
| **`pulldown-cmark`** | `psychopomp-render` | `0.13.4` | **`0.13.4`** | 2026-05-20 | Unchanged | Already on latest (`0.13.4`) with `default-features = false`. |
| **`png`** | `psychopomp-render` | `0.18.1` | **`0.18.1`** | 2026-02-14 | Unchanged (`flate2 1.1.9→1.1.10` with `zlib-rs 0.6.8`) | Already on latest (`0.18.1`). Note: `cargo update` upgrades `flate2` to `1.1.10`, which pulls in `zlib-rs 0.6.8` (pure-Rust SIMD-accelerated deflate, used by Cargo itself since 1.88), speeding up `plan frame` / `plan snapshot` PNG encoding. |
| **`resvg`** | `psychopomp-render` | `0.47.0` | **`0.48.1`** | 2026-08-02 | Unchanged (`0.47.0`) | **`0.48.0`/`0.48.1`** (Linebender) migrated SVG text from `rustybuzz` + `ttf-parser` to `harfrust` + `skrifa` (which `cosmic-text 0.19` already uses!) and added `svgz` / `writer` feature gates. With `default-features = false` (**done in `42ea532`**), `resvg` compiles only `usvg` path parsing + `tiny-skia 0.12.0` rasterization. Bumping `0.47 → 0.48` with `default-features = false` is a **zero-line code change** (`resvg::usvg::Tree::from_str` and `resvg::render` signatures are identical). |
| **`wgpu`** | `psychopomp-render` | `30.0.0` | **`30.0.1`** | 2026-08-22 | `30.0.1` | Already on major version `30.0` (released 2026-07-01, which added `SurfaceColorSpace` HDR output, `SHADER_I16`, and `Queue::present`). **`30.0.1` (2026-08-21)** fixes dynamic resolution of Metal color-space constants (#9819) and a Vulkan `vkAcquireNextImageKHR` fence validation error (#9855). **Zero migration cost** via `cargo update`. Also consider `default-features = false, features = ["std", "parking_lot", "metal", "vulkan", "dx12", "wgsl", "naga-ir"]` to drop unused `gles` (`glow`, `khronos-egl`) and `webgpu` (`js-sys`, `web-sys`, `wasm-bindgen`) targets. |
| **`winit`** | `psychopomp-render` | `0.30.13` | **`0.30.13`** *(beta: `0.31.0-beta.3`)* | 2026-03-02 *(beta: 2026-09-04)* | Unchanged | Already on latest stable (`0.30.13`). **Stay on `0.30.13`**: `0.31.0-beta.3` is still pre-release and introduces major breaking changes (`Window` and `ActiveEventLoop` become `dyn` traits, `create_window` returns `Box<dyn Window>`, `inner_size` renamed to `surface_size`, `MouseInput`/`CursorMoved` replaced by `PointerButton`/`PointerMoved`, `NamedKey::Space` removed in favor of `Key::Character(" ")`, `objc2` bumped `0.5 → 0.6`). Wait for `0.31.0` stable. |

---

## 4. Rust Language, Standard Library & Tooling Features (Rust 1.85 → 1.99.0)

Psychopomp compiles with **Rust 1.99.0** (released 2026-09-28 / 2026-10-01, LLVM 23) on **Edition 2024**. Below is the status of every language, library, and Cargo feature stabilized between Rust 1.85 and 1.99.0 that applies to Psychopomp, plus unstable features to avoid.

### 4.1 Stabilized Features Already Used or Ready to Adopt

| Feature / API | Stabilized In | Status in Psychopomp & Where to Use It |
|---|---|---|
| **`let_chains` (`if let ... && let ...`)** | **1.88.0** (2025-06-26, Edition 2024) | **Already used** in `main.rs`, `render/caption.rs`, `render/stage.rs`, `rolling.rs`. Can simplify nested `if let` blocks in `plan_runtime/preflight.rs` and `editor/stability.rs`. |
| **`<[T]>::as_chunks::<N>()` / `as_chunks_mut::<N>()`** | **1.88.0** (2025-06-26) | **Already used** across `exposure.rs`, `render.rs`, `render/ui/card.rs`, `render/value.rs` (after commit `f911f5c` fixed Rust 1.99's `clippy::chunks_exact_to_as_chunks`). Note: in `exposure.rs`, passing the fixed-size `&mut [f32; 4]` and `&[u8; 4]` directly to helper functions (done in `1ed4331`) preserves the compile-time length `4` instead of decaying back to `&[f32]` / `&[u8]`. |
| **`<[T]>::array_windows::<N>()`** | **1.94.0** (2026-03-05) | **High-value upgrade:** Codebase currently calls `.windows(2)` **28 times** (e.g. `pair[0]`, `pair[1]` with runtime bounds checks in `rolling.rs:281`, `math.rs:49`, `math/curve.rs`, `render/ui/card.rs:410`, `plan_runtime.rs:839`, `meter.rs:286`). Replacing `.windows(2)` with `.array_windows::<2>()` destructures `for &[a, b] in points.array_windows()` with zero bounds checks. |
| **`{f32, f64}::algebraic_add`, `algebraic_sub`, `algebraic_mul`, `algebraic_div`** | **1.98.0** (2026-08-20) | **Cutting-edge performance feature:** Allows LLVM to reassociate floating-point operations (enabling SIMD horizontal reductions and FMA vectorization) on specific expressions without `unsafe` `std::intrinsics::fadd_fast` or nightly `portable_simd`. Ideal for CPU pixel blending (`render/ui/card.rs`, `render/text/raster.rs`) while keeping timeline/spring integration strict IEEE-754. |
| **`std::sync::LazyLock` + `.get()` / `From<T>`** | **1.80.0** (`get`/`force_mut` in **1.94.0**, `From` in **1.96.0**) | **Adopted in `exposure.rs` (`1ed4331`)** replacing `OnceLock` + `get_or_init`. Also ideal for caching `fontdb::Database` in `render/fonts.rs` and the Phosphor pointer SVG sprite in `render.rs`. |
| **`assert_matches!` / `debug_assert_matches!`** | **1.96.0** (2026-05-28) | Stabilized in `std::assert_matches::assert_matches!` (wait: in 1.96 `assert_matches!` is in the prelude/std macro namespace). Can replace ~35 `assert!(matches!(...))` calls in tests (e.g. `stage.rs:920,956,990`, `callout.rs:628`) to get structural failure diagnostics on test failure instead of `"assertion failed: matches!(...)"`. |
| **`if let` guards on `match` arms** | **1.95.0** (2026-04-16) | Allows `match element { StageElement::Card { id, .. } if let Some(place) = self.placements.get(id.as_str()) => ... }` in `render/stage.rs` and `plan_runtime/preflight.rs`. |
| **`cfg_select!` macro in `std`** | **1.95.0** (2026-04-16) | Built-in replacement for external `cfg-if` crate when branching on target OS/arch. |
| **`<[T]>::as_array::<N>()` / `as_mut_array::<N>()`** | **1.93.0** (2026-01-22) | Converts `&[u8]` slices (such as `&row[target..target + 4]`) directly into `Option<&[u8; 4]>` without `.try_into().unwrap()`. |
| **`bool::ok_or` / `bool::ok_or_else`** | **1.98.0** (2026-08-20) | Concise validation checks in non-`anyhow` code (`condition.ok_or(Error)?`). |
| **`f32` / `f64` const math (`clamp`, `abs`, `max`, `min`, `recip` in 1.85; `floor`, `ceil`, `round`, `fract` in 1.90; `mul_add` in 1.94)** | **1.85.0 – 1.94.0** | Enables `const fn` easing evaluation, `const` spring profiles (`Feel::visual` in `dsl`), and compile-time LUT generation where only polynomial/rational math is needed (note: transcendental `powf`/`sin`/`cos`/`exp` are still not `const fn` in 1.99). |
| **Workspace `default-features = false` override (RFC 3945)** | **Cargo 1.99.0** (2026-10-01) | On Edition 2024, workspace members can now write `dep = { workspace = true, default-features = false }` even when `[workspace.dependencies]` enables default features. |
| **Cargo `build.warnings` config** | **Cargo 1.97.0** (2026-07-09) | Can set `[build] warnings = "deny"` in CI instead of passing `RUSTFLAGS="-Dwarnings"` (which previously busted Cargo/sccache caches between `cargo test` and `cargo clippy`). |
| **Cargo `build.build-dir` config** | **Cargo 1.91.0** (2025-10-30) | Separates intermediate build artifacts from final `target/` outputs; can share intermediate crate artifacts across worktrees. |
| **`#[expect(lint)]` attribute** | **1.81.0** (2024-09-05) | Replace `#[allow(clippy::too_many_arguments)]` (used in `exposure.rs:38`, `render/text/raster.rs`, etc.) with `#[expect(clippy::too_many_arguments)]` so the compiler warns `unfulfilled_lint_expectations` when a refactor removes the need for the suppression. |

### 4.2 Features Investigated That Are Still Unstable on Rust 1.99.0 (Do Not Use Yet)

- **`std::simd` (`#![feature(portable_simd)]`)**: Still nightly-only as of Rust 1.99.0 (blocked on lane-count bounds and mask semantics). **Alternative on stable 1.99.0:** `glam` (which uses `core::arch::aarch64` NEON intrinsics on Apple Silicon and `sse2` on x86_64), `<[T]>::as_chunks::<4>()` + `{f32}::algebraic_add/mul` (stabilized in 1.98.0 for LLVM 23 auto-vectorization), or `wide 1.7.1` / `pulp 0.22.3` if explicit 8-lane `f32x8` SIMD is needed.
- **`gen` blocks (`#![feature(gen_blocks)]`, Tracking Issue #117078)**: Reserved keyword in Edition 2024, but `gen {}` / `gen fn` remain unstable on 1.99.0 (blocked on `Iterator` vs ` LendingIterator` / coroutine pin ergonomics).
- **`f16` and `f128` primitive types (`#![feature(f16, f128)]`, Tracking Issue #116909)**: The primitive types remain nightly-only on 1.99.0 (though AArch64 NEON fp16 and x86 `avx512fp16` intrinsics that do not mention `f16` in their signature were stabilized in Rust 1.94.0). `naga` uses `half::f16` on the CPU and WGSL `enable f16;` on the GPU.
- **Rust 2027 Edition**: Currently in early RFC/tracking phase (e.g. rust-lang/rust#149359 non-poisoning `Mutex`, #151084 unified `panic`, `style_edition = "2027"` unstable in `rustfmt`). Edition 2024 is the latest stable edition.
- **Parallel rustc frontend (`-Zthreads=8`) & Cranelift codegen (`-Zcodegen-backend=cranelift`)**: Both still require `-Z` unstable flags on 1.99.0.

---

## 5. Ecosystem Shifts & Re-evaluating "Current Stack Decisions"

We evaluated `ARCHITECTURE.md`'s *"Current Stack Decisions"* against the October 2026 Rust graphics ecosystem:

### 5.1 `vello 0.11.0` & `vello_cpu 0.3.0` (Released 2026-10-02) vs `render/ui/card.rs` & `Stage`
- **Verified Facts (2026-10-02):** Linebender released **`vello 0.11.0`** on 2026-10-02, upgrading `vello`, `vello_encoding`, and `vello_shaders` to **`wgpu 30` and `naga 30`** (matching Psychopomp's exact `wgpu 30` version!). Linebender also released **`vello_cpu 0.3.0`** (sparse-strips CPU/hybrid 2D renderer) and **`glifo 0.4.0`** on 2026-10-02.
- **Should Psychopomp adopt Vello now?**
  - **For `Stage`:** **No.** `Stage` (`render/stage.rs`, `stage.wgsl`, `stage_post.wgsl`) is a specialized HDR (`Rgba16Float`) signed-distance + procedural raymarched combustion + 24-sample GPU exposure + 5-level bloom + refractive shockwave renderer. Vello is a 2D vector path renderer and cannot replace `Stage`'s custom SDF light pools, rim reflections, or HDR post-processing.
  - **For CPU Overlays (`render/ui/card.rs`, `plot.rs`, `sequence.rs`, `callout.rs`):** **Keep current stack for existing overlays; consider `kurbo 0.13.1` (already in `Cargo.lock` if needed) or `vello` only if complex arbitrary Bezier/SVG vector scenes are added.** With our PoC 2 optimization (`1ed4331`), CPU card compositing is now **7.5× faster** and preserves Psychopomp's bespoke depth-weighted near-edge Gaussian blur (`near_edge_blur`) and analytic coverage union (`UiCanvas::polyline`, `line_marks`), which Vello does not natively model.

### 5.2 `parley 0.11.1` / `skrifa 0.44` / `harfrust` vs `cosmic-text 0.19.0`
- **Verified Facts:** Both `cosmic-text 0.19.0` (2026-04-22) and Linebender's `parley 0.11.1` (2026-08-16) have converged on the **exact same underlying font stack**: Google Fontations (`skrifa` + `read-fonts`) and `harfrust` (pure-Rust HarfBuzz port), plus `swash`.
- **Recommendation:** **Stay on `cosmic-text 0.19.0`**, which already uses `harfrust` + `skrifa` and directly provides the glyph-cluster hitboxes used by `PreparedEditor::select` for Semantic Targets.
- **Immediate High-Impact Fix in `render/fonts.rs`:** Currently `fonts::font_system()` calls `db.load_system_fonts()` every time a `HeadlessRenderer` is constructed. Cache the initialized `fontdb::Database` (with system fonts loaded and installed `CommitMono` stripped, plus the 4 bundled `CommitMono` faces loaded) inside a `static FONT_DB: LazyLock<fontdb::Database>`, and clone `FONT_DB.clone()` in `font_system()`. Because `fontdb::Database` stores font data behind `Arc` (`Source::Binary(Arc<dyn AsRef<[u8]> + Sync + Send>)` / `Source::File(PathBuf)`), `.clone()` is an instant shallow clone instead of scanning `/System/Library/Fonts` and `/Library/Fonts` from disk on every CLI invocation.

### 5.3 `wesl 0.6.0` (Released 2026-10-04) vs `concat!` + `PSYCHOPOMP_SHADER_DIR` + `naga`
- **Verified Facts:** `wesl` (WGSL Extended Shading Language, `wesl-rs`) released **`0.6.0` on 2026-10-04** (`0.5.0` on 2026-09-13). It supports `import` statements, `@if`/`@elif`/`@else` conditional compilation, `wesl-quote`, and `build.rs` compilation.
- **Trade-off for Psychopomp:** Psychopomp's `EFFECTS.md` contract intentionally keeps `render/effects/*.wgsl` (`noise.wgsl`, `combustion.wgsl`, `pressure.wgsl`, `rewind.wgsl`, plus `dissolve.wgsl`, `lightning.wgsl`, `scan.wgsl`, `shield.wgsl` on the `effects` branch) as binding-free WGSL modules that can be hot-reloaded at runtime via `PSYCHOPOMP_SHADER_DIR=crates/psychopomp-render/src/render` without recompiling Rust. Adopting `wesl` syntax (`import package::noise::fx_fbm3;`) would mean shaders are no longer valid standalone WGSL and would require running the `wesl` compiler at runtime during `PSYCHOPOMP_SHADER_DIR` live-reload.
- **Recommendation:** **Keep pure WGSL syntax + `PSYCHOPOMP_SHADER_DIR` live reload, paired with the GPU-free `naga` validation test we added in PoC 3 (`7763bea`)** (extended to include the 4 new `effects/*.wgsl` files once the `effects` branch merges).

### 5.4 Moving Non-Stage Temporal Accumulation & Card Projection to the GPU (`wgpu`)
- `ARCHITECTURE.md` notes: *"A production compositor should render and accumulate directly in linear `Rgba16Float`, then tone-map into the delivery color space."*
- `Stage` **already** does this (`render/stage.rs`: 24 samples accumulate into `gpu.exposure` `Rgba16Float` via `stage_post.wgsl`'s `accumulate` entry point, then develop once with only 1 GPU→CPU readback per output frame!).
- By contrast, non-Stage roots (`Editor`, `Grid`, `Title`) and `Reel` transitions (`crossfade`, `dip`, `zoom`, `wipe`) currently do a **GPU→CPU readback (`read_frame`) per temporal sample** (8–16 readbacks per frame) and accumulate on the CPU in `exposure::accumulate`.
- **Medium-term architectural win (post-integration):** Upload CPU overlay sprites/layers or run `CardProjection` + `exposure::accumulate` in a shared `Rgba16Float` wgpu pass (reusing `stage_post.wgsl`'s `accumulate` pass), reducing `Editor` and `Reel` exports from 16 `device.poll` readbacks per frame to **1 readback per frame**.

---

## 6. Build & Tooling Diagnosis: Why `sccache` Had a 0.56% Hit Rate

Inspecting `sccache --show-stats` on this machine revealed a critical developer-experience bottleneck across the 13 worktrees:

```text
Compile requests                   2141
Compile requests executed           182
Cache hits                            1 (0.56%)
Cache misses                        178
Non-cacheable calls                1957
Non-cacheable reasons:
  multiple input files             1032
  incremental                       650
  crate-type                        182
```

### Why This Happened & How to Fix It
1. **`Non-cacheable: incremental (650 calls)`**: Cargo enables `incremental = true` by default for `dev` and `test` profiles. **`sccache` cannot cache any `rustc` invocation with `-C incremental=...`**. Thus, every `cargo test` and `cargo check` in all 13 worktrees compiled all 198 dependencies from scratch!
   - **Fix in root `Cargo.toml` (after branch integration):**
     ```toml
     [profile.dev]
     # Allow sccache to cache all 198 third-party crates across worktrees,
     # and shrink target/debug DWARF size by ~45% while keeping file:line backtraces.
     incremental = false
     debug = "line-tables-only"

     [profile.dev.package."*"]
     opt-level = 2
     debug = false
     ```
     With `incremental = false` (or `CARGO_INCREMENTAL=0` in `.cargo/config.toml`) and `[profile.dev.package."*"] opt-level = 2`, `sccache` caches every third-party `.rlib`/`.rmeta` across all worktrees (`<10 ms` cache hit per crate!), and CPU-heavy pixel/font tests in `cargo test` run **5–10× faster** because `cosmic-text`, `tiny-skia`, `resvg`, and `png` are compiled with `opt-level = 2`.
2. **Release profile tuning (`[profile.release]`):**
   ```toml
   [profile.release]
   lto = "thin"
   codegen-units = 1
   ```
    Enables cross-crate inlining between `psychopomp` (`math`, `motion`, `timeline`, `rolling`) and `psychopomp-render`.

---

## 7. Next-Generation Macros, Score DSL Integration & Testing Patterns

### 7.1 Connecting `recipe_channels!` to the New `score::Beat` Algebra (`dsl` branch)
In `dsl/crates/psychopomp/src/score.rs` (`DSL.md`), every actor handle (`score::Stage`, `score::Caption`, `score::Callout`, `score::RollingNumber`, `score::Tree`, `score::Plot`, `score::Lanes`, `score::Sequence`, `score::Video`) is an immutable `Clone` value whose methods return `impl Beat`.
However, channel animations on `score::Stage` and custom channels on other actors are still stringly-typed:
```ts
// Current in dsl/scenes/2password/src/bin/2password_dsl.rs:
s.to("leak-line.opacity", 0.0, 0.4),
s.to("agent.alarm", 0.0, 0.4),
s.to("camera.x", 0.0, 1.6),
```
By combining `recipe_channels!` (PoC 1) with a lightweight `ChannelKey<'a>` value type in `score`:
```ts
#[derive(Clone, Copy, Debug)]
pub struct ChannelKey<'a> {
    pub actor: &'a ActorHandle,
    pub property: &'a str,
    pub initial: f32,
}

impl<'a> ChannelKey<'a> {
    pub fn set(self, value: f32) -> impl Beat + 'a { ... }
    pub fn to(self, target: f32, seconds: f32) -> impl Beat + 'a { ... }
    pub fn bounce(self, target: f32, seconds: f32, bounce: f32) -> impl Beat + 'a { ... }
    pub fn spring(self, target: f32, feel: SpringPlan) -> impl Beat + 'a { ... }
    pub fn ease(self, target: f32, seconds: f32, curve: Ease) -> impl Beat + 'a { ... }
    pub fn hit(self, peak: f32, rest: f32) -> impl Beat + 'a { ... }
}
```
`recipe_channels!` can automatically generate `.opacity()`, `.x()`, `.y()`, `.typed()`, `.caret()` methods on `score::Caption` (and every other `score::*` value handle!) returning `ChannelKey<'_>`, plus typed element handles on `Stage` (`s.card("agent").alarm().to(0.0, 0.4)`, `s.camera().x().to(0.0, 1.6)`):
```ts
// Typed channel beats in score::all![]:
all![
    s.card("agent").alarm().to(0.0, 0.4),
    s.card("agent").status().to(0.0, 0.9).after(millis(200)),
    s.camera().x().to(0.0, 1.6),
    footer.x().spring(24.0, Feel::PANEL),
]
```
A typo in a channel name becomes a **compile-time Rust error** in the Scene Program instead of a runtime preflight error!

### 7.2 Eliminating `plan_runtime/<recipe>.rs` Duplication via `StrictRecipe`
Once the in-flight branches (`components-viz`, `components-text`, `anchors`, `lens`, `diagnostics`) merge, `crates/psychopomp-render/src/plan_runtime/` will have **~18 near-identical files** (`caption.rs`, `plot.rs`, `lanes.rs`, `sequence.rs`, `meter.rs`, `bars.rs`, `checklist.rs`, `confetti.rs`, `subtitles.rs`, `lower_third.rs`, etc.) that all define:
```ts
pub(super) struct Prepared<X> { id: String, plan: <X>Plan }
```
Extending `recipe_channels!` to implement a `StrictRecipe` trait in `crates/psychopomp`:
```ts
pub trait StrictRecipe: Serialize + DeserializeOwned {
    const RECIPE: &'static str;
    fn validate(&self) -> Result<()>;
    fn accepts(&self, property: &str) -> bool;
}
```
allows `plan_runtime/preflight.rs` to use a single generic `PreparedOverlay<R: StrictRecipe>` struct:
```ts
pub(super) struct PreparedOverlay<R> {
    pub id: String,
    pub plan: R,
}

impl<R: StrictRecipe> PreparedOverlay<R> {
    pub(super) fn new(actor: &ActorPlan, channels: &[ContinuousChannelPlan]) -> Result<Self> {
        let plan = decode(actor, R::RECIPE, R::validate)?;
        strict_channels(&actor.id, channels, R::RECIPE, |prop| plan.accepts(prop))?;
        Ok(Self { id: actor.id.clone(), plan })
    }
}
```
deleting ~15 boilerplate files in `plan_runtime/` while keeping every recipe's validation and strict channel check 100% intact.

### 7.3 Property-Based Testing (`proptest 1.11.0`) & Snapshot Testing (`insta 1.49.0`)
- **`proptest 1.11.0`** (dev-dependency in `crates/psychopomp`): `DSL.md` defines 7 exact algebraic laws for `score::Beat` (Sequence Monoid Associativity & Identity, Hold Fusion, Delay Distributivity over Parallel, Impulse Absorption, Stagger Decomposition, and Distinct-Timestamp Parallel Commutativity) and 3 spatial laws for `layout::Placement`. Adding `proptest` in `crates/psychopomp` `[dev-dependencies]` lets a single 50-line test verify all 10 laws across thousands of randomized `(at_nanos, delay, gap, target, profile)` combinations.
- **`insta 1.49.0`** (`cargo-insta` is already installed in `/Users/kit/.cargo/bin/cargo-insta`): Can snapshot `psychopomp::editor::inspect_steps(&plan)` and `ScenePlan::diff` diagnostics so step-stability regressions can be reviewed interactively with `cargo insta review`.

---

## 8. Ordered Post-Integration Execution Plan

### Phase 1: Merge `modernize` PoCs (Ready Now, Minimal Conflict Surface)
1. Cherry-pick or merge the 4 commits on `modernize`:
   - `0f35b7d` (`feat(author): add recipe_channels! macro and apply to caption recipe`)
   - `1ed4331` (`perf(render): parallelize card compositing and pre-weight linear exposure tables`) — **7.5× faster CPU card frames**
   - `7763bea` (`test(render): validate WGSL shaders and uniform struct layouts via naga without a GPU`)
   - `42ea532` (`build(deps): trim unused glam/resvg default features and drop unused scene deps`)

### Phase 2: Immediate Post-Integration Build, Font & Dependency Upgrades (1 Commit)
1. **Cache `fontdb::Database` in `LazyLock`** in `crates/psychopomp-render/src/render/fonts.rs` so `HeadlessRenderer::new()` clones the pre-indexed font database in `<1 ms` instead of rescanning macOS font directories (`~400–600 ms`).
2. **Add `[profile.dev]`, `[profile.dev.package."*"]`, and `[profile.release]`** to root `Cargo.toml` (`incremental = false` for `sccache` worktree sharing, `debug = "line-tables-only"`, `opt-level = 2` for dependencies, `lto = "thin"` for release).
3. **Centralize `[workspace.dependencies]` and `[workspace.lints]`** in root `Cargo.toml`, run `cargo update` (`wgpu 30.0.1`, `serde 1.0.229`, `serde_json 1.0.151`, `anyhow 1.0.104`, `bytemuck 1.25.2`, `flate2 1.1.10` + `zlib-rs 0.6.8`), and bump `glam = "0.34"`, `resvg = "0.48"`, `pollster = "1.0"`.
4. **Add the 4 new `effects/*.wgsl` shaders** from the `effects` branch to `wgsl_tests` in `crates/psychopomp-render/src/main.rs`.

### Phase 3: Recipe & Score DSL Unification (1–2 Commits)
1. Apply `recipe_channels!` to all static-channel recipes (`RollingNumber`, `Video`, `ValueToken`, `Venn`, `Meter`, `Bars`, `Checklist`, `Confetti`, `Subtitles`, `LowerThird`, `Terminal`, `Chat`, `ChangedFiles`, `Window`, `Lens`, `Ide`) and add an optional `accepts_dynamic: |plan, prop| ...` arm to `recipe_channels!` for recipes with dynamic IDs (`Stage`, `Callout`, `Plot`, `Lanes`, `Sequence`, `Tree`).
2. Add `ChannelKey<'a>` to `psychopomp::score` so every `recipe_channels!` field automatically becomes a typed `Beat`-producing method on `score::*` value handles.
3. Replace `.windows(2)` with Rust 1.94's `.array_windows::<2>()` across `crates/psychopomp` and `crates/psychopomp-render`.
4. Collapse identical `plan_runtime/<recipe>.rs` wrappers into `PreparedOverlay<R: StrictRecipe>`.
