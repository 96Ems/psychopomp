# Psychopomp Public API Coherence Audit (`docs/COHERENCE.md`)

This document audits the entire authoring and recipe surface of `crates/psychopomp`—both on `main` and across all ten parallel worktrees in flight (`authoring`, `anchors`, `camera`, `components-text`, `components-viz`, `diagnostics`, `effects`, `lens`, `stage-forms`, `transitions`, and `dsl`)—and records what has already been unified on `dsl` alongside the remaining integration steps.

---

## 1. Coherent Architecture: `PlanBuilder` Owns State, Actors Are Values

Prior to `dsl`, actor handles had inconsistent ownership models and signatures:
- `RollingNumberActor` and `TreeActor` cached mutable copies of their recipe plans (`plan`, `open`, `scroll`) inside the handle struct, requiring `&mut self` and preventing handles from being cloned or shared across closures.
- `StageActor`, `CaptionActor`, `CalloutActor`, `PlotActor`, `LanesActor`, `SequenceActor`, and `VideoActor` took `&mut self` on every method even though they never mutated `self`.
- `SequenceActor`, `CaptionActor`, `RollingNumberActor`, `TreeActor`, and `LanesActor` lacked `actor(&self) -> &ActorHandle` and `id(&self) -> &str`.

### Fixed on `dsl`:
1. **`PlanBuilder` is the single source of truth (`author.rs`):**
   - Added `PlanBuilder::actor_data::<T>(&self, actor)` and `PlanBuilder::latest_literal(&self, actor, property)`.
   - Moved `RollingNumberActor`'s `plan` state and `TreeActor`'s `plan`/`open`/`scroll` state into `PlanBuilder`.
   - Every `*Actor` handle in `crates/psychopomp` is now `#[derive(Clone, Debug)]`, stateless after declaration, and exposes `actor(&self) -> &ActorHandle` and `id(&self) -> &str`.
2. **Value Actor Handles in `psychopomp::score` (`score.rs`):**
   - `Stage`, `Caption`, `Callout`, `RollingNumber`, `Tree`, `Plot`, `Lanes`, `Sequence`, and `Video` wrap their respective `*Actor` handles (with `::declare`, `From<*Actor>`, and `.beats()`).
   - Every choreography method on these handles takes `&self` and returns `impl Beat + '_` playing against `&mut PlanBuilder` (`scene.at(time, beat)`), so all actors and `sound(...)` compose inside a single `all![...]` or `.then(...)` tree without per-actor contexts or escape hatches.
3. **`caption::hide` Resting Opacity Bug Fixed (`caption.rs`):**
   - Previously, `caption::hide` (shared by `CaptionActor`, `RollingNumberActor`, and `TreeActor`) called `scene.channel(actor, "opacity", 0.0)`. Hiding an actor that started visible at `t = 0` (without an earlier `show`) declared `opacity` with `initial: 0.0` and sprung `0.0 → 0.0`!
   - Fixed to `scene.channel(actor, "opacity", 1.0)`. When `show` or `type_in` was called earlier, `PlanBuilder::channel` preserves the existing `0.0` initial value; when `hide` is the first opacity write, it starts at the renderer's resting default (`1.0`).
4. **Out-of-Order Parallel Event Sorting Fixed (`author.rs`):**
   - `PlanBuilder::finish` stably sorts each continuous and state channel's events by `at_nanos` (`sort_by_key`), so parallel beats touching the same channel at distinct timestamps validate and commute without triggering `unordered-events`, while equal timestamps preserve source order.

---

## 2. Inventory of All 21 Actor Handles Across Worktrees

| Branch | Actor Handle | Score Value Handle (`dsl`) | Identity Accessors | Visibility API | Status |
| --- | --- | --- | --- | --- | --- |
| `main` / `dsl` | `StageActor` | `score::Stage` | `actor()`, `id()`, `plan()` | `fade_in` / `fade_out` (`authoring`) | Complete in `dsl` + `authoring` |
| `main` / `dsl` | `CaptionActor` | `score::Caption` | `actor()`, `id()` | `show()`, `hide()`, `type_in()` | Complete in `dsl` (`hide` bug fixed) |
| `main` / `dsl` | `CalloutActor` | `score::Callout` | `actor()`, `id()` | `show()`, `hide()` | Complete in `dsl` |
| `main` / `dsl` | `RollingNumberActor` | `score::RollingNumber` | `actor()`, `id()` | `show()`, `hide()` | Complete in `dsl` (state in `PlanBuilder`) |
| `main` / `dsl` | `TreeActor` | `score::Tree` | `actor()`, `id()`, `model()` | `show()`, `hide()` | Complete in `dsl` (state in `PlanBuilder`) |
| `main` / `dsl` | `PlotActor` | `score::Plot` | `actor()`, `id()` | `show(axes_s)`, `hide()` | Complete in `dsl` |
| `main` / `dsl` | `LanesActor` | `score::Lanes` | `actor()`, `id()` | `show(s)`, `hide()` | Complete in `dsl` |
| `main` / `dsl` | `SequenceActor` | `score::Sequence` | `actor()`, `id()` | `animate("opacity", ...)` | Complete in `dsl` |
| `main` / `dsl` | `VideoActor` | `score::Video` | `actor()`, `id()`, `plan()` | `fly_in()`, `hide()` | Complete in `dsl` |
| `anchors` | `TextActor` | *(add `score::Text` on merge)* | `actor()` (add `id()`) | `show`, `hide` | Uses `psychopomp::anchor` |
| `components-text` | `TerminalActor` | *(add `score::Terminal` on merge)* | `actor()` (add `id()`) | `show`, `hide` | New overlay in `components-text` |
| `components-text` | `ChatActor` | *(add `score::Chat` on merge)* | `actor()` (add `id()`) | `show`, `hide` | New overlay in `components-text` |
| `components-text` | `ChangedFilesActor` | *(add `score::ChangedFiles` on merge)* | `actor()` (add `id()`) | `show`, `hide` | New overlay in `components-text` |
| `components-viz` | `BarsActor` | *(add `score::Bars` on merge)* | `id()` (add `actor()`) | `show`, `hide` | New overlay in `components-viz` |
| `components-viz` | `ChecklistActor` | *(add `score::Checklist` on merge)* | `id()` (add `actor()`) | `show`, `hide` | New overlay in `components-viz` |
| `components-viz` | `ConfettiActor` | *(add `score::Confetti` on merge)* | `id()` (add `actor()`) | `burst` | New overlay in `components-viz` |
| `components-viz` | `MeterActor` | *(add `score::Meter` on merge)* | `id()` (add `actor()`) | `show`, `hide` | New overlay in `components-viz` |
| `components-viz` | `SubtitlesActor` | *(add `score::Subtitles` on merge)* | `id()` (add `actor()`) | `show`, `hide` | New overlay in `components-viz` |
| `diagnostics` | `DiffEditor`, `DiagnosticActor`, `HoverActor`, `CursorActor` | *(add `score::*` wrappers on merge)* | `actor()` (add `id()`) | `show`, `hide` | New IDE overlays in `diagnostics` |
| `lens` | `LensActor` | *(add `score::Lens` on merge)* | `actor()` (add `id()`) | `show`, `hide` | New magnifier overlay in `lens` |

---

## 3. Cross-Branch Collisions to Reconcile at Merge Time

1. **Duplicate 2D Noise in `crates/psychopomp/src/math/random.rs` (`effects` vs `transitions`):**
   - `effects` adds `pub fn lattice_noise2(x: f32, y: f32, salt: u32) -> f32` and `pub fn lattice_fbm2(x: f32, y: f32, salt: u32) -> f32`.
   - `transitions` adds `pub fn value_noise2(point: Vec2, salt: u32) -> f32` and `pub fn fractal_noise2(point: Vec2, octaves: u32, salt: u32) -> f32`.
   - **Action on merge**: Keep `value_noise2(Vec2, u32)` and `fractal_noise2(Vec2, u32, u32)` as the primary functions in `math::random`, and delegate `lattice_noise2` / `lattice_fbm2` to them.

2. **`authoring` Timing Helpers vs `dsl` (`score.rs`):**
   - `dsl` intentionally includes `Narration::reading`, `StagePost::RESTRAINED`, `StatusText::new`, and `StageElement::{card, orb, beam, packet, label, ring, ...}` with identical signatures and behavior to `authoring`.
   - **Action on merge**: When merging `authoring` and `dsl`, deduplicate those `impl` blocks in `narration.rs` and `stage.rs`, re-export `CueTime` in `author.rs` (aliasing `PlanTime`), and back `score::Feel` with `SpringPlan::{PANEL, CONTENT, SNAP, CAMERA, LIVELY}`.

3. **`anchors` vs `diagnostics` (`CursorAnchorPlan`) and `stage-forms` (`callout.rs`):**
   - `anchors` extracts `psychopomp::anchor::{AnchorPlan, Edge, move_to}` for `callout`, `caption`, `rolling`, and `text`.
   - **Action on merge**: Route `diagnostics`'s `CursorAnchorPlan` and `stage-forms`'s new `StageElement` outlines through `psychopomp::anchor`.

4. **Pre-existing Rust 1.99 Clippy Lint in `psychopomp-render` (`clippy::chunks_exact_to_as_chunks`):**
   - On both `main` and `dsl`, `crates/psychopomp-render` triggers Rust 1.99's new `clippy::chunks_exact_to_as_chunks` lint on `.chunks_exact(4)` / `.chunks_exact_mut(4)` calls in `exposure.rs`, `render.rs`, `still.rs`, `grid.rs`, `header.rs`, `theme.rs`, `value.rs`, `component_prototype.rs`, `raster.rs`, and `card.rs`.
   - **Action**: Replace `.chunks_exact(4)` / `.chunks_exact_mut(4)` in `psychopomp-render` with `.as_chunks::<4>().0` / `.as_chunks_mut::<4>().0` in a dedicated `psychopomp-render` housekeeping commit after the feature branches merge.

---

## 4. Recommended Integration Order

1. **`authoring`** → `main`: lands `StagePlan::channel_default` (`resting` channels), `SpringPlan` feels, `Narration::reading`, and `StageElement` constructors.
2. **`dsl`** → `main`: lands `psychopomp::score`, `psychopomp::layout`, `DSL.md`, `docs/COHERENCE.md`, `PlanBuilder` state accessors (`actor_data`, `latest_literal`, event sorting), `caption::hide` fix, and the byte-identical proof binaries (`hello_dsl`, `2password_dsl`).
3. **`anchors`** → `main`: lands `psychopomp::anchor` and `TextActor`.
4. **`stage-forms`**, **`effects`**, **`camera`** → `main`: lands Stage shapes, procedural effects, and `CameraRig`; expose `CameraRig` methods on `score::Stage`.
5. **`transitions`**, **`lens`** → `main`: deduplicate `math::random` 2D noise.
6. **`diagnostics`**, **`components-text`**, **`components-viz`** → `main`: land new overlay recipes and add `score::*` value handle wrappers for each new actor.
