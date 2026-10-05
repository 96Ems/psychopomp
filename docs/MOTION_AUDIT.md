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
- [ ] **Reel Transitions & Compare** (`psychopomp-transitions`, `psychopomp-compare`)
- [ ] **Overlays, Callouts, Anchors & Lens** (`psychopomp-callouts`, `psychopomp-anchors`, `psychopomp-loupe`)
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

---

## 2. Shared Motion Token System

_Pending audit synthesis._

---

## 3. Polish Changes & Before/After Evidence

_Pending implementation._
