# Psychopomp Public API Coherence Audit (`docs/COHERENCE.md`)

This document audits the entire authoring and recipe surface of `crates/psychopomp`—both on `main` and across all ten parallel worktrees in flight (`authoring`, `anchors`, `camera`, `components-text`, `components-viz`, `diagnostics`, `effects`, `lens`, `stage-forms`, `transitions`, and `dsl`)—and prescribes the exact unifications, renames, bug fixes, and merge order to make the library read as if one person designed it in one sitting.

---

## 1. Inventory of Actor Handles (21 Total Across Branches)

With the parallel branches in flight, `psychopomp` grows from **9** actor handles on `main` to **21** actor handles:

| Branch | Actor Handle | `declare`Plan Arg | Identity Accessor | Visibility API | Resting Opacity Bug? | Motion API |
| --- | --- | --- | --- | --- | --- | --- |
| `main` | `StageActor` | `&StagePlan` | `actor()` | `fade_in`/`fade_out` (in `authoring`) | Fixed in `authoring` (`resting` table); on `main` `to()` defaulted opacity to `0.0`! | `to`, `bounce`, `spring`, `ease`, `set`, `hit`, `kick`, `jolt`, `settle_in`, `connect`, `send`, `twang`, `land`, `type_in`, `clock`, `clock_for` |
| `main` | `SequenceActor` | `&SequencePlan` | *(none!)* | `show` / `hide` missing (uses `animate("opacity", ...)`) | Caller passes manual `initial: f32` to `animate` | `animate(prop, initial, at, target, s)`, `reveal`, `fade_row`, `strike`, `emphasize` |
| `main` | `CaptionActor` | `&CaptionPlan` | *(none!)* | `show`, `hide` | **Yes:** `caption::hide` declares `opacity` with initial `0.0`, so hiding a caption that started visible (without an earlier `show`) fades `0.0 → 0.0`! | `type_in(at, cps, caret_hold)` |
| `main` | `RollingNumberActor` | `RollingNumberPlan` *(by value)* | *(none!)* | `show`, `hide` | **Yes:** delegates to `caption::hide` (`initial = 0.0`) | `roll(at, value) -> Result<()>` |
| `main` | `TreeActor` | `TreePlan` *(by value)* | *(none!)* | `show`, `hide` | **Yes:** delegates to `caption::hide` (`initial = 0.0`) | `open`, `close`, `highlight`, `set`, `scroll_to`, `reveal` |
| `main` | `PlotActor` | `&PlotPlan` | `id()` | `show(at, s)` *(no `hide`!)* | `fade(series, at, opacity)` declares series opacity at `1.0` | `draw`, `fade`, `ride`, `stop_ride`, `velocity`, `mark` |
| `main` | `LanesActor` | `&LanesPlan` | *(none!)* | `show(at, s)` *(no `hide`!)* | `show` declares `opacity` at `0.0` | `scrub`, `emphasize` |
| `main` | `CalloutActor` | `&CalloutPlan` | *(none!)* | `show(at)`, `hide(at)` | `hide` declares `opacity` at `1.0` (different from `caption::hide`!) | `move_to(at, anchor)`, `emphasize(at)` |
| `main` | `VideoActor` | `VideoPlan` *(by value)* | `actor()`, `plan()` | `fly_in(at)`, `hide(at)` *(no `show`!)* | `hide` declares `opacity` at `1.0` | `focus(at, region, s)`, `unfocus(at, s)` |
| `anchors` | `TextActor` | `&TextPlan` | `actor()` | `show`, `hide` | Uses `caption::show` / `hide` | `move_to(at, anchor)` |
| `components-text` | `TerminalActor` | `TerminalPlan` | `actor()` | `show`, `hide` | Uses `window` helpers | `run`, `type_command`, `output`, `scroll` |
| `components-text` | `ChatActor` | `ChatPlan` | `actor()` | `show`, `hide` | Uses `window` helpers | `post`, `type_message`, `react` |
| `components-text` | `ChangedFilesActor` | `ChangedFilesPlan` | `actor()` | `show`, `hide` | Uses `window` helpers | `reveal_row`, `highlight_row` |
| `components-viz` | `BarsActor` | `&BarsPlan` | `id()` | `show`, `hide` | Channel-based | `select_series`, `emphasize` |
| `components-viz` | `ChecklistActor` | `&ChecklistPlan` | `id()` | `show`, `hide` | Channel-based | `check`, `strike`, `activate` |
| `components-viz` | `ConfettiActor` | `&ConfettiPlan` | `id()` | `burst(at)` | Clock-based (`-1.0` default) | `burst` |
| `components-viz` | `MeterActor` | `&MeterPlan` | `id()` | `show`, `hide` | Channel-based | `to`, `ease` |
| `components-viz` | `SubtitlesActor` | `&SubtitlesPlan` | `id()` | `show`, `hide` | Transcript-driven | `from_spoken` |
| `diagnostics` | `DiffEditor` | `Diff` | `actor()`, `editor()` | `enter` | Panel channels | `semantic_target`, `inlay` |
| `diagnostics` | `DiagnosticActor` | `&DiagnosticPlan` | `actor()` | `show`, `hide` | Squiggle + message channels | `show`, `hide`, `emphasize` |
| `diagnostics` | `HoverActor` | `&HoverPlan` | `actor()` | `show`, `hide` | Card fade | `show`, `hide` |
| `diagnostics` | `CursorActor` | `&CursorPlan` | `actor()` | `show`, `hide` | Caret blink + anchor weights | `move_to`, `select` |
| `lens` | `LensActor` | `&LensPlan` | `actor()` | `show`, `hide` | Magnifier glass channels | `move_to`, `zoom` |

---

## 2. Six Systemic Inconsistencies & Bugs Found

### 2.1 The Undeclared-Channel Default Bug (`0.0` vs `1.0`)

1. **`StageActor::to` / `ease` / `bounce` / `set` on `main`**:
   Called `self.channel(scene, property, 0.0)` for every undeclared property, even though `Stage` defaults `opacity`, `scale`, `content`, `spin`, `draw`, `typed`, `sweep`, and `post.exposure` to `1.0` and clocks (`burst`, `mark`, `spinner`, `release`, `post.rewind`) to `-1.0`. Calling `stage.to(&mut scene, "link.opacity", at, 0.0, 0.5)` on a visible wire declared `link.opacity` with `initial: 0.0`, so the wire popped invisible at `t = 0`!
   - **Status**: Fixed in `authoring` (`1de90c2`) via `StagePlan::channel_default(property)`.
2. **`caption::hide(scene, actor, at_nanos)` (used by `CaptionActor`, `RollingNumberActor`, and `TreeActor`)**:
   ```ts
   pub(crate) fn hide(scene: &mut PlanBuilder, actor: &ActorHandle, at_nanos: u64) {
       let opacity = scene.channel(actor, "opacity", 0.0); // <-- BUG if `show` was not called first!
       scene.spring(&opacity, at_nanos, 0.0, 0.3, 0.0);
   }
   ```
   In `plan_runtime/caption.rs`, `rolling.rs`, and `tree.rs`, the renderer defaults an undeclared `opacity` channel to `1.0` (so an actor declared without `show` starts visible at `t = 0`). If an author declares a visible `TreeActor` or `CaptionActor` at `t = 0` and later calls `.hide(scene, at)`, `caption::hide` declares `opacity` with `initial: 0.0` and springs `0.0 → 0.0`—making the actor invisible for the entire scene! By contrast, `CalloutActor::hide` and `VideoActor::hide` correctly pass `1.0` as the fallback initial value when `opacity` was not already declared by `show`.
   - **Fix**: Change `caption::hide` to `let opacity = scene.channel(actor, "opacity", 1.0);`. Because `PlanBuilder::channel` keeps the existing initial value (`0.0`) if `show` was called earlier, passing `1.0` works whether `show` was called first or not!

### 2.2 Out-of-Order Events on Parallel Channel Writes (`unordered-events`)

`PlanBuilder::set_to`, `spring_to`, `spring_with`, and `ease` previously appended directly to `channel.events: Vec<TrackEventPlan>`, while `ScenePlan::validate()` requires `events` to be sorted by `at_nanos`. Whenever two parallel choreographic branches (or two narration-keyed lines in a scene) touched the same channel out of chronological order, `scene.finish()` failed with `unordered-events`.
- **Status**: Fixed in `dsl` (`e0bd415`): `PlanBuilder::finish` stably sorts each continuous and state channel's events by `at_nanos` (`sort_by_key`), preserving source order for equal timestamps while making out-of-order parallel writes valid and commutative.

### 2.3 Cross-Branch Duplication Between Sibling Worktrees

Reading all ten sibling worktrees exposed three direct collisions that must be reconciled on merge:

1. **2D Value/Lattice Noise in `crates/psychopomp/src/math/random.rs`**:
   - Branch `effects` adds `pub fn lattice_noise2(x: f32, y: f32, salt: u32) -> f32` and `pub fn lattice_fbm2(x: f32, y: f32, salt: u32) -> f32`.
   - Branch `transitions` adds `pub fn value_noise2(point: Vec2, salt: u32) -> f32` and `pub fn fractal_noise2(point: Vec2, octaves: u32, salt: u32) -> f32` in the exact same file (`math/random.rs`)!
   - **Unification**: Keep `value_noise2(point: Vec2, salt: u32)` and `fractal_noise2(point: Vec2, octaves: u32, salt: u32)` in `math::random` (since `math` uses `glam::Vec2` everywhere), and make `lattice_noise2`/`lattice_fbm2` thin aliases or migrate `effects` to call `value_noise2(vec2(x, y), salt)` and `fractal_noise2(vec2(x, y), 4, salt)`.

2. **Anchor Vocabulary (`CalloutSide` / `CalloutAnchorPlan` vs `psychopomp::anchor`)**:
   - Branch `anchors` extracts `psychopomp::anchor::{AnchorPlan, AnchorTarget, Edge, move_to, weight_property}` so `CalloutActor`, `CaptionActor`, `RollingNumberActor`, and `TextActor` can all pin to Stage elements and editor Semantic Targets.
   - Branch `diagnostics` adds `CursorAnchorPlan` and `HoverSide` in `crates/psychopomp/src/ide.rs`.
   - Branch `stage-forms` touches `callout.rs` to support new `StageElement` outlines.
   - **Unification**: Route `diagnostics`'s `CursorAnchorPlan` and `stage-forms`'s element outlines through `psychopomp::anchor::AnchorPlan` and `Edge` so every pinned overlay in the engine shares one anchor type.

3. **Timing Helpers in `authoring` vs Composable Algebra in `dsl`**:
   - Branch `authoring` adds `MILLISECOND`, `millis`, `PlanTime::not_before`, `author::stagger`, `author::spread`, `SpringPlan::{PANEL, CONTENT, SNAP, CAMERA, LIVELY}`, and `StageActor::{spring, fade_in, fade_out, send_arriving, connect_contacting}`.
   - Branch `dsl` adds `psychopomp::score` (`Score`, `StageScore`, `Beat`, `Span`, `CueTime`, `chain!`, `all!`, `stagger`, `stagger_indexed`, `Feel`, `score::stage::*`) and `psychopomp::layout` (`Placement`, `row`, `column`, `spread_x`).
   - **Unification**:
     - Re-export `MILLISECOND`, `millis`, and `spread` from `author` and `score`.
     - Unify `PlanTime` (`authoring`) and `CueTime` (`dsl`) so both `u64` and `Span` implement `.not_before(earliest)`, `.after(delta)`, and `.early(lead)`.
     - Use `SpringPlan::{PANEL, CONTENT, SNAP, CAMERA, LIVELY}` directly as the backing constants for `score::Feel`.

### 2.4 Inconsistent Handle Method Signatures

Across existing actor handles:

1. **Identity accessors**:
   - `StageActor` and `VideoActor` have `pub fn actor(&self) -> &ActorHandle`.
   - `PlotActor` has `pub fn id(&self) -> &str` (and no `actor()`).
   - `SequenceActor`, `CaptionActor`, `RollingNumberActor`, `TreeActor`, `LanesActor`, `CalloutActor` have **neither** `actor()` nor `id()`!
   - **Fix**: Add `pub fn actor(&self) -> &ActorHandle` and `pub fn id(&self) -> &str` to every `*Actor` struct.

2. **`declare` ownership of `*Plan`**:
   - `StageActor`, `SequenceActor`, `CaptionActor`, `PlotActor`, `LanesActor`, `CalloutActor` take `&Plan` by reference.
   - `RollingNumberActor`, `TreeActor`, `VideoActor` take `Plan` by value (because `RollingNumberActor::roll` and `TreeActor::set` mutate their owned `Plan` and call `scene.replace_actor_data`).
   - **Rule**: Keep `&Plan` for immutable-payload recipes and owned `Plan` (or `impl Borrow<Plan>`) where the handle appends to the recipe payload, or accept `impl Borrow<XPlan>` on immutable ones so both value and reference work at the call site.

3. **Whole-actor property animation (`StageActor::to` vs `SequenceActor::animate`)**:
   - `StageActor::to(&mut self, scene, property, at_nanos, target, seconds)` uses recipe channel defaults.
   - `SequenceActor::animate(&mut self, scene, property, initial, at_nanos, target, seconds)` forces the caller to pass `initial: f32` every time!
   - **Fix**: Add `SequencePlan::channel_default(property)` (`opacity: 1.0`, `lifelines: 1.0`, `x: 0.0`, `y: 0.0`, `row.*.opacity: 1.0`, `row.*.reveal: 0.0`, `row.*.strike: 0.0`, `participant.*.opacity: 1.0`, `participant.*.emphasis: 0.0`) and give `SequenceActor` the same `to(scene, property, at_nanos, target, seconds)` signature as `StageActor`.

4. **Parameter Order in `TreeActor`**:
   - `TreeActor::highlight(scene, path, at_nanos, seconds)` puts `at_nanos` after `path`.
   - `TreeActor::set(scene, path, value, at_nanos)` puts `at_nanos` *after* `value` (at the end), whereas `PlanBuilder::set(channel, at_nanos, value)`, `StageActor::set(scene, prop, at_nanos, value)`, and `RollingNumberActor::roll(scene, at_nanos, value)` all put `at_nanos` *before* the target value!
   - **Fix**: Standardize on `(scene, target_id, at_nanos, value_or_params...)` or beat constructors `tree::set(path, value)` where `at_nanos` is supplied by the `Score`!

5. **Return values of Phased Actions**:
   - `StageActor::settle_in`, `connect`, `send`, `type_in`, `CaptionActor::type_in`, `PlotActor::draw`, `PlotActor::ride` return `u64` (the completion timestamp in nanoseconds).
   - `CalloutActor::show` (draws leader over `0.42s`, reveals label at `+280ms`), `SequenceActor::reveal` (`0.6s`), `TreeActor::open`/`close` (`0.45s`), and `VideoActor::fly_in` (`0.8s`) return `()` instead of their completion timestamp!
   - **Fix**: Have phased entrances (`CalloutActor::show`, `SequenceActor::reveal`, `TreeActor::open`/`close`, `VideoActor::fly_in`) return `u64` (their completion timestamp in nanoseconds), and expose corresponding `Beat` constructors in `psychopomp::score` that return `Span(at, end)`.

---

## 3. Concrete Refactoring Table for Integration

| Priority | Module / Type | Current State | Target State | Breaking to JSON? |
| --- | --- | --- | --- | --- |
| **P0 (Bug)** | `caption::hide` | `scene.channel(actor, "opacity", 0.0)` | `scene.channel(actor, "opacity", 1.0)` | Only fixes broken un-shown hides |
| **P0 (Bug)** | `PlanBuilder::finish` | Raw `.events` order fails `validate()` on parallel out-of-order writes | Stably sort `events` by `at_nanos` in `finish()` (implemented in `dsl`) | **No** (no-op on sorted plans) |
| **P0 (Bug)** | `StageActor::to` / `ease` / `set` | Defaults undeclared channels to `0.0` on `main` | Use `StagePlan::channel_default` (implemented in `authoring`) | Only fixes broken fade-outs |
| **P1 (Merge)** | `math::random` | Duplicate 2D noise in `effects` (`lattice_noise2`) and `transitions` (`value_noise2`) | Unify on `value_noise2(Vec2, u32)` and `fractal_noise2(Vec2, u32, u32)` | **No** |
| **P1 (Merge)** | `author` + `score` | `authoring` has `PlanTime::not_before` + `author::stagger`; `dsl` has `CueTime` + `score::stagger` | Re-export `CueTime` from `author` (with `PlanTime` alias); keep `author::stagger` for closure style and `score::stagger` for `Beat` style | **No** |
| **P1 (Merge)** | `anchor` + `callout` + `ide` | `anchors` introduces `psychopomp::anchor`; `diagnostics` has `CursorAnchorPlan` | Use `anchor::AnchorPlan` and `anchor::Edge` across `callout`, `caption`, `rolling`, `text`, and `ide` | **No** |
| **P2 (API)** | All 21 `*Actor` structs | Missing `actor()` / `id()` on 11 handles | Add `pub fn actor(&self) -> &ActorHandle` and `pub fn id(&self) -> &str` to all handles | **No** |
| **P2 (API)** | `CalloutActor::show`, `SequenceActor::reveal`, `TreeActor::open`/`close`, `VideoActor::fly_in` | Return `()` | Return `u64` completion timestamp so they chain in `Score` and imperative code alike | **No** |
| **P2 (API)** | `SequenceActor::animate` | Requires manual `initial: f32` parameter | Add `SequenceActor::to(scene, prop, at, target, seconds)` using recipe defaults | **No** |

---

## 4. Recommended Merge & Migration Order

To integrate all 11 worktrees cleanly with minimal conflict resolution:

1. **`authoring`** → `main` first: establishes `StagePlan::channel_default`, `SpringPlan` feels, `Narration::reading`, and `MILLISECOND` / `millis`.
2. **`dsl`** → `main` second: adds `crates/psychopomp/src/score.rs`, `crates/psychopomp/src/layout.rs`, `DSL.md`, `docs/COHERENCE.md`, `PlanBuilder::finish` event sorting, and the byte-identical proof binaries (`hello_dsl`, `2password_dsl`). Wire `score::Feel` to `SpringPlan::{PANEL, CONTENT, SNAP, CAMERA, LIVELY}`.
3. **`anchors`** → `main` third: lands `psychopomp::anchor` and `TextActor`, unifying `callout`, `caption`, and `rolling` anchors.
4. **`stage-forms`**, **`effects`**, **`camera`** → `main` fourth: all three extend `crates/psychopomp/src/stage.rs` and `crates/psychopomp-render/src/render/stage.rs` (new shapes, lightning/dissolve/shield effects, and `CameraRig`).
5. **`transitions`**, **`lens`** → `main` fifth: deduplicate `math::random::value_noise2` vs `lattice_noise2` when merging `transitions` after `effects`.
6. **`diagnostics`**, **`components-text`**, **`components-viz`** → `main` sixth: self-contained new overlay recipes (`ide`, `terminal`, `chat`, `changed_files`, `bars`, `checklist`, `confetti`, `meter`, `subtitles`). Add `.score(&mut scene)` and `Beat` constructors for each new overlay handle as a follow-up pass.
