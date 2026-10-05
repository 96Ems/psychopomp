# Psychopomp Motion Audit & Polish

This document tracks the frame-by-frame motion audit of Psychopomp's showrooms, recipes, and narrated films on `main` (`cc48084`), defines the unified motion token vocabulary, and links before/after contact strips and video studies under `output/polish/`.

## Audit Criteria

Every showroom and component is evaluated against the house motion rules (`explainer-motion/SKILL.md`, `explainer-motion/TECHNIQUES.md`, `PRIOR_ART.md`):

1. **Entrance / exit asymmetry**: Exits should be simpler and faster than entrances; avoid floaty linear or long-tailed exits.
2. **Overshoot discipline**: No bounce on persistent layout, opacity, blur, or utility transitions; reserve deliberate overshoot (`PANEL` 0.12, `LIVELY` 0.2) for rigid physical panels and hero arrivals.
3. **Overlapping action & container/content lag**: Rigid containers lead their ink by ~60–65 ms; secondary labels follow primary lines or bars; blur resolves faster than or alongside opacity/scale rather than lingering over readable text.
4. **Continuity & velocity**: Pure functions of time; interrupted moves preserve position and velocity; minimum-jerk (`smootherstep`) for exact point-to-point glides; `cubic-bezier(0.45, 0.0, 0.2, 1.0)` for wire/stroke draw-on.
5. **Pacing, stagger, and dead frames**: ~120 ms row/card stagger; transitions without dead holds or abrupt starts/stops; flashes with instant attack and convex decay.

---

## 1. Component & Showroom Audit

- [x] **Stage, Camera, Forms & Effects** (`psychopomp-camera`, `psychopomp-stage-forms`, `psychopomp-effects-showroom`)
- [x] **Reel Transitions & Compare** (`psychopomp-transitions`, `psychopomp-compare`)
- [x] **Overlays, Callouts, Anchors & Lens** (`psychopomp-callouts`, `psychopomp-anchors`, `psychopomp-loupe`)
- [ ] **Text Surfaces & Lower Third** (`psychopomp-text-surfaces`)
- [ ] **Visualization Overlays & Subtitles** (`psychopomp-viz-components`, `psychopomp-charts`, `psychopomp-tree`, `psychopomp-rolling-number`)
- [ ] **Editor, Diagnostics, Footage & Films** (`psychopomp-diagnostics`, `psychopomp-footage`, `scenes/2password`, `scenes/psychopomp-intro`)

### 1.1 Stage, Camera, Forms & Effects

**Evidence sheets:**
- `output/polish/audit/camera-overview.jpg` (`0:32:1.6`)
- `output/polish/audit/camera-whip.jpg` (`21.8:22.7:0.06`)
- `output/polish/audit/stage-forms-overview.jpg` (`0:15:0.75`)
- `output/polish/audit/stage-forms-morph.jpg` (`5.5:7.1:0.1`)
- `output/polish/audit/stage-forms-burst.jpg` (`12.4:13.6:0.08`)
- `output/polish/audit/effects-showroom-overview.jpg` (`0:21:1.05`)
- `output/polish/audit/effects-replace-scan.jpg` (`14.2:19.6:0.3`)

**Findings:**
1. **`StageActor::scan` uses `Ease::Linear` (`crates/psychopomp/src/stage.rs:2288`)**:
   - In `effects-replace-scan.jpg` (`18.40s..19.60s`), the verification scan line sweeps down `config v2` at constant velocity (`Ease::Linear`), starting and stopping with an abrupt velocity step at the card's top and bottom edges. A scan sweep should accelerate smoothly onto the card and decelerate into the bottom edge (`DRAW_CURVE` / `Ease::Smootherstep`).
2. **Label collision & dead air in `effects-showroom` (`scenes/effects-showroom/src/lib.rs`)**:
   - At `15.70s` in `effects-replace-scan.jpg`, `stage.materialize("new", at, 1.6)` types directly over the still-fading `stage.dissolve("old", at)` (`stagstage.dissolve("old", at)`), violating the staging rule that two readable payloads in the same spot must not collide. The same overlap occurs between `charge`/`zap` (`2.55s..2.80s`) and `raise`/`zap` (`9.30s..9.55s`) because `say` uses a slow `0.25s Smoothstep` fade that overlaps the next label's start.
   - Between `15.40s` and `16.60s` in `effects-replace-scan.jpg`, the frame sits nearly empty for ~1.1 s after `old` finishes burning at `BURN = 1.1 s` because `formed` waits `0.8 s` after `gone` and `StageActor::materialize` starts at `dissolve::DURATION` (`2.05 s`), spending its first ~350 ms in the sparse late-ash tail before flakes visibly gather.
3. **Inconsistent `Smoothstep` vs `Smootherstep` in `StageActor` and `CameraRig` (`crates/psychopomp/src/stage.rs`, `stage/camera.rs`)**:
   - `CameraRig::drift`, `CameraRig::handheld`, `CameraRig::whip`'s zoom-streak envelope, `StageActor::raise`, `StageActor::lower`, and `StageActor::amount` (`charge`, `hum`) use cubic `Ease::Smoothstep` (which has non-zero acceleration $\pm 6$ at both ends and a second-derivative cusp at the midpoint of `whip`), whereas the house minimum-jerk standard is quintic `Ease::Smootherstep`.
4. **Destination cards pop in mid-whip in `scenes/camera` (`camera-whip.jpg` `21.92s..22.28s`)**:
   - During the `0.6 s` whip pan to `queue` and `drain` (`21.80s..22.40s`), `settle_in` is scheduled at `whip + 0.25s` and `whip + 0.39s`. As the camera streaks into the right cluster (`21.98s..22.16s`), the destination is empty and `drain` pops in after the pan has nearly landed.
5. **Flat form entrances and unreactive burst in `scenes/stage-forms` (`stage-forms-overview.jpg`, `stage-forms-burst.jpg`)**:
   - `slab`, `store`, and `cache` enter with a bare `0.8 s` opacity ease (`0.5s..1.7s`) with no scale or y-drift, reading flat compared to `client` and `gateway`.
   - When `cache` bursts at `12.50s` (`stage-forms-burst.jpg` `12.56s..12.88s`), `cache-name` ("cache") and the dashed `fill` path wait `300 ms` before starting a slow `0.8 s` fade, leaving the label sitting inside the fireball, and `gateway` directly above the blast takes no `shock_kick` or rim reaction when the refractive wave crosses it at `12.72s`.

### 1.2 Reel Transitions & Compare

**Evidence sheets:**
- `output/polish/audit/transitions-part1.jpg` (`1.2:20.0:1.0`)
- `output/polish/audit/transitions-part2.jpg` (`20.5:38.5:1.0`)
- `output/polish/audit/transitions-slide.jpg` (`15.5:16.5:0.08`)
- `output/polish/audit/transitions-flash.jpg` (`31.8:32.7:0.06`)
- `output/polish/audit/transitions-leak.jpg` (`35.8:37.5:0.12`)
- `output/polish/audit/compare-overview.jpg` (`0:9:0.5`)

**Findings:**
1. **`slide_travel` front-loads 91% of travel into the first 40% and crawls for 320 ms (`crates/psychopomp/src/plan/transition.rs:42`)**:
   - In `transitions-slide.jpg`, the slide starts at `15.74s` and has already covered 91% of the frame height by `16.06s` (`t = 0.4`), then barely moves across `16.14s`, `16.22s`, `16.30s`, `16.38s`, and `16.46s` (5 consecutive frames, `320 ms`). `slide_travel` delegates to `dynamics::settle(0.0, 1.0, 0.0, 1.0, progress)` whose $\omega = 10$ reaches peak speed `3.68×` at `t = 0.1` and spends `[0.5, 1.0]` crawling the last 4% of the distance.
2. **`flash` transition has a delayed cubic attack and harsh flat whiteout (`crates/psychopomp/src/plan/transition.rs:190`, `render/transition/light.rs:18`)**:
   - In `transitions-flash.jpg` (`31.98s..32.46s`), `flash_intensity` uses `rise³` before `FLASH_PEAK = 0.24`, so almost nothing happens at `32.04s` (`+60 ms`, intensity `0.02`), then it spikes to a flat gray/white slab at `32.10s..32.22s` (`FLASH_GAIN = 6.0`). A smoother attack (`smoothstep(t / FLASH_PEAK)`) with restrained gain (`3.5`) builds into the cut without a single-frame pop.
3. **`light-leak` transition scorches 85% of the frame white for 500 ms (`crates/psychopomp-render/src/render/transition/light.rs:66-104`, `plan/transition.rs:202`)**:
   - In `transitions-leak.jpg` (`36.28s..36.88s`), `LEAK_GAIN = 2.4` across 5 overlapping full-frame glows sums to `~13.0` in linear light, turning nearly the entire 1080p frame solid white-yellow so neither the outgoing diagram nor the incoming `"psychopomp"` title is readable. `leak_strength` also uses `sin(πt)^1.5`, which has infinite second derivative at both ends instead of a smooth bell (`smootherstep(2t)` / `smootherstep(2 - 2t)`).
4. **`ink_threshold` uses cubic `smoothstep` instead of `smootherstep` (`crates/psychopomp/src/plan/transition.rs:161`)**:
   - Every other spatial transition (`push_travel`, `iris_radius`, `flip_turn`, `cube_turn`, `ReelZoom::matched`, `ReelWipePlan::position`) uses quintic `smootherstep` so acceleration is continuous at both ends; `ink_threshold` uses `smoothstep`, causing the ink blot front to start and stop with a nonzero acceleration step.

### 1.3 Overlays, Callouts, Anchors & Lens

**Evidence sheets:**
- `output/polish/audit/callouts-overview.jpg` (`0:13:0.65`)
- `output/polish/audit/callouts-enter.jpg` (`1.2:2.5:0.1`)
- `output/polish/audit/anchors-overview.jpg` (`0:18:0.9`)
- `output/polish/audit/loupe-overview.jpg` (`0:14:0.7`)
- `output/polish/audit/loupe-capsule.jpg` (`3.2:6.4:0.2`)

**Findings:**
1. **Callout leader finishes drawing 80–100 ms before its label appears (`crates/psychopomp/src/callout.rs:394-438`)**:
   - In `callouts-enter.jpg`, at `1.50s` (`retries 3×`), `1.90s` (`cache hit · 2 ms`), and `2.20s` (`a fixed point stays put`), the leader line and its horizontal elbow shelf are already 90–95% drawn and sit pointing at empty space before the label starts fading in (`LABEL_DELAY = 280 ms` vs `DRAW_SECONDS = 0.42 s` on `DRAW_CURVE`, which reaches 82% at `220 ms`). Reducing `LABEL_DELAY` to `180 ms` lets the label rise in as the leader tip rounds the elbow.
2. **Broken initial channel defaults when `hide` is called without `show` (`callout.rs:442`, `lens.rs:462`, `ide.rs:638,709`)**:
   - `CalloutActor::hide` declares `draw` and `label` with initial value `0.0` (though `PreparedCallout` defaults both to `1.0` at rest!).
   - `LensActor::hide` declares `presence` with initial value `0.0` (though `LensPlan::glass` defaults `presence` to `1.0` at rest!).
   - `HoverActor::hide` declares `presence` with initial `0.0` and `CursorActor::hide` declares `opacity` with initial `0.0`.
   - Calling `.hide()` on any of these actors when they started visible at time zero makes them vanish from `t = 0` instead of hiding at `at_nanos`.
3. **Desynchronized resize vs glide/slide springs on `LensActor` (`crates/psychopomp/src/lens.rs:475-523`)**:
   - In `loupe-capsule.jpg` (`5.60s..6.20s`), when the loupe moves from `schedule` (capsule `560×96`) to `limit` (circle `300×300`), `resize` uses a `0.55 s` spring with `0.12` bounce while `move_to` and `slide` use a `0.75 s` critically damped spring (`0.0` bounce). At `6.00s`, the lens has already collapsed into a circle while still mid-flight between lines, and the `0.12` bounce on `width`/`height` causes the loupe rim to wobble after changing shape. Aligning `resize`, `magnify`, `focus`, `slide`, and `move_to` to coherent zero-bounce profiles (`0.65 s` glide / `0.55 s` reshape, `0.0` bounce) makes the glass move and reshape as one physical optic.
4. **Entrance / exit symmetry on `caption::hide` and `TextActor::hide` (`crates/psychopomp/src/caption.rs:254-266`, `text.rs:176-187`)**:
   - `caption::show` and `TextActor::show` fade in over `0.35 s` and rise `10 px` over `0.45 s`; `hide` uses `0.30 s` — only `50 ms` faster than entry. Tightening `hide` to `0.24 s` (`Feel::EXIT`) gives exits a noticeably cleaner, snappier release than entries, consistent with `window::dismiss` (`0.25 s`).

---

## 2. Shared Motion Token System

_Pending audit synthesis._

---

## 3. Polish Changes & Before/After Evidence

_Pending implementation._
