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
- [x] **Text Surfaces & Lower Third** (`psychopomp-text-surfaces`)
- [x] **Visualization Overlays & Subtitles** (`psychopomp-viz-components`, `psychopomp-charts`, `psychopomp-tree`, `psychopomp-rolling-number`)
- [x] **Editor, Diagnostics, Footage & Films** (`psychopomp-diagnostics`, `psychopomp-footage`, `scenes/2password`, `scenes/psychopomp-intro`)

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

### 1.4 Text Surfaces & Lower Third

**Evidence sheets:**
- `output/polish/audit/text-surfaces-overview.jpg` (`0:50:2.5`)
- `output/polish/audit/lower-third-show.jpg` (`0.8:1.8:0.08`)
- `output/polish/audit/lower-third-hide.jpg` (`4.8:5.45:0.05`)
- `output/polish/audit/text-surfaces-chat.jpg` (`20.5:34.0:0.85`)
- `output/polish/audit/text-surfaces-files.jpg` (`43.2:51.2:0.5`)

**Findings:**
1. **`LowerThird` text only slides 1–2 characters (`size * 1.4`) and gets guillotined in mid-air while the bar retracts early (`crates/psychopomp-render/src/render/lower_third.rs:78-102`, `crates/psychopomp/src/lower_third.rs:173-193`)**:
   - In `lower-third-show.jpg` (`1.20s..1.36s`) and `lower-third-hide.jpg` (`5.00s..5.20s`), the slide distance is fixed at `plan.size * 1.4` (`64 px`) while `"opencode"` is `~210 px` wide, and the clip edge sits `12.5 px` to the right of the bar. As a result, at `1.20s` and `5.05s` the word reads `"pencode"` with `"o"` hard-clipped in open space to the right of the bar while `"ncode"` merely fades in place.
   - In `lower-third-hide.jpg` (`5.15s..5.20s`), `bar` begins retracting at `at + 220 ms` while `name` (`at + 70 ms`, `0.36 s` spring) is still ~25% visible, so at `5.15s..5.20s` the bar shrinks to a stub beside still-visible text.
   - Sliding each line by its full measured advance (`sprite.advance + plan.size * 0.4`), placing the clip edge flush with the bar's right edge, and sequencing `hide` so the bar retracts as the name finishes clearing makes the text genuinely emerge from and withdraw behind the bar.
2. **Whole-thread vertical bounce on `ChatActor::say` (`crates/psychopomp/src/chat.rs:30-31, 547`)**:
   - `SAY_BOUNCE = 0.12` on `message.<id>.reveal` causes `ChatPlan::room` (`lerp(waiting, said, reveal)`) to overshoot `1.0` on every sent message. Because `ChatPlan::layout` stacks all older messages upward from the composer (`top -= gap + room`), every message in the thread bounces up past its resting y-coordinate and sinks back down whenever a new message lands (`text-surfaces-chat.jpg`). Persistent layout growth should be critically damped (`SAY_BOUNCE = 0.0`, `SAY_SECONDS = 0.45`), matching `Tree` and `Terminal`, with restrained pop reserved for reaction pills (`REACT_BOUNCE = 0.2`).

### 1.5 Visualization Overlays & Subtitles

**Evidence sheets:**
- `output/polish/audit/viz-components-overview.jpg` (`0:52:2.6`)
- `output/polish/audit/viz-checklist.jpg` (`0.4:12.4:0.75`)
- `output/polish/audit/viz-meters.jpg` (`13.2:22.8:0.6`)
- `output/polish/audit/viz-bars.jpg` (`23.8:33.4:0.6`)
- `output/polish/audit/viz-subtitles.jpg` (`35.0:51.0:1.0`)
- `output/polish/audit/subtitles-swap.jpg` (`40.35:40.95:0.04`)
- `output/polish/audit/charts-overview.jpg` (`0:22:1.1`)
- `output/polish/audit/tree-overview.jpg` (`0:12:0.6`)
- `output/polish/audit/tree-fold-scroll.jpg` (`5.7:6.6:0.08`)
- `output/polish/audit/rolling-number-overview.jpg` (`0:8:0.4`)
- `output/polish/audit/rolling-redirect.jpg` (`1.75:2.35:0.05`)

**Findings:**
1. **`Readout` smears stationary higher-place digits and double-exposes fast decimal wheels (`crates/psychopomp/src/readout.rs:155-225`, `crates/psychopomp-render/src/render/viz/readout.rs:105-110`)**:
   - In `viz-meters.jpg` (`15.00s..21.00s`), `draw_readout` computes smear from `wheel_rate(format, place, velocity)` (`|v| * 10^(decimals - place)`), even though a higher place (`place > 0`) only turns while every lower place is rolling over from `9` (`below >= unit - 1.0`). When `cpu` springs from `96` to `41`, the tens digit is smeared even while parked on `4` or `8`.
   - When the lowest place turns faster than ~8 units/s (such as the tenths digit of `upload` sweeping at `15.2 %/s`, `152 faces/s`), `odometer`'s `ROLL = 0.35` step-and-hold snaps between integers across temporal samples inside a single frame's shutter, producing a double-exposed vertical bar (`10.||%`, `34.||%`, `54.||%`). Gating `wheel_rate` / smear by whether the place is actually carrying (`wheel.fract() != 0.0`) keeps stationary digits crisp!
2. **Value overshoot on `MeterActor::set` and `BarsActor::set` causes readouts to count past the target and roll backward (`crates/psychopomp/src/meter.rs:359`, `bars.rs:485,541`)**:
   - `MeterActor::set` uses `bounce = 0.12` and `BarsActor::set` uses `bounce = 0.08`. Because their `Readout`s are pure functions of the sampled `value`, springing `cpu` to `74%` or `cold start` to `1,840 ms` makes the odometer count up past the target (`76%`, `1,865 ms`) and then roll backward. Similarly, `BarsActor::sort` uses `bounce = 0.1` on `slot`, causing sorted rows to overshoot their destination row and bob back. Critically damped springs (`bounce = 0.0`) on `MeterActor::set`, `BarsActor::set`, and `BarsActor::sort` make readouts and row re-sorts monotonic.
3. **Subtitles page swap has a zero-opacity text gap and a double-pumped backing morph (`crates/psychopomp/src/subtitles.rs:466-586`)**:
   - In `subtitles-swap.jpg` at `40.71s`, the subtitle backing box sits completely empty with zero text! During a direct swap (`page.swaps`), `page[i]` fades out over `[hide - FADE_OUT, hide]` (reaching `0.0` at `hide`) while `page[i+1]` fades in over `[show, show + FADE_IN]` (starting from `0.0` at `show == hide`).
   - Worse, `SubtitleLayout::backing` computes `k = smootherstep(leaving * 0.5)` before `hide` and `k = smootherstep(0.5 + entering * 0.5)` after `hide`, where `leaving` and `entering` are already `smoothstep`ped. Because `smoothstep'(1.0) = 0` and `smoothstep'(0.0) = 0`, the backing box's width/height morph comes to a complete stop (`dk/dt = 0`) at `k = 0.5` (`40.71s`) and then re-accelerates! Using a single linear parameter across the swap window inside `smootherstep` eliminates the mid-swap stall, and overlapping the outgoing/incoming page fades slightly across the swap boundary eliminates the blank-box frame.
4. **Desynchronized fold vs scroll springs in `TreeActor` (`crates/psychopomp/src/tree.rs:34-36`)**:
   - In `tree-fold-scroll.jpg` (`5.90s..6.50s`), `OPEN_SECONDS = 0.45` while `SCROLL_SECONDS = 0.55`. When `open` and `reveal` are triggered together at `5.90s`, the container expands faster than the scroll window moves, pushing the bottom rows into the bottom fade mask around `6.02s..6.18s` before the scroll catches up. Matching `SCROLL_SECONDS` to `OPEN_SECONDS` (`0.45 s`) keeps fold expansion and scroll tracking locked together.

### 1.6 Editor, Diagnostics, Footage & Films

**Evidence sheets:**
- `output/polish/audit/diagnostics-overview.jpg` (`0:13:0.65`)
- `output/polish/audit/footage-overview.jpg` (`0:28:1.4`)
- `output/polish/audit/footage-toss.jpg` (`0.2:1.2:0.08`)
- `output/polish/audit/hello-overview.jpg` (`0:4:0.25`)
- `output/polish/audit/2password-overview.jpg` (`0:56:2.8`)
- `output/polish/audit/psychopomp-intro-overview.jpg` (`0:34:1.7`)

**Findings:**
1. **`CursorActor::select` uses `Ease::CubicInOut` (mid-flight acceleration jump) and rigid `300 ms` pre-wait (`crates/psychopomp/src/ide.rs:768-769`)**:
   - `CursorActor::select` sweeps `head` with `Ease::CubicInOut` (which has an acceleration discontinuity at `t = 0.5`, explicitly warned against in `explainer-motion/SKILL.md`) instead of `Ease::Smootherstep`.
   - `FootageActor::drift` (`crates/psychopomp/src/footage.rs:1073`) and `FootageActor::treat` (`footage.rs:1117`) also use `Ease::CubicInOut` instead of `Ease::Smootherstep`.
2. **Shouted text `"ANY FRAME!"` collides with the hero orb in `scenes/psychopomp-intro` (`psychopomp-intro-overview.jpg` `10.20s`, `11.90s`)**:
   - In `scenes/psychopomp-intro/src/main.rs:125`, `"shout-frame"` is placed at `[960.0, 330.0, 160.0]` (`size: 150`) while the orb sits at `[960.0, 500.0, 0.0]` (`radius: 140`, top at `y = 360`), so the bottom of `"ANY FRAME!"` is occluded by the dark orb shell. Raising `"shout-frame"` to `y = 260.0` (and `"shout-order"` to `y = 740.0`) clears the orb silhouette cleanly.

---

## 2. Shared Motion Token System

Defined in `crates/psychopomp/src/author.rs`, `crates/psychopomp/src/score.rs`, and `crates/psychopomp/src/math/easing.rs`, grounded in `.agents/skills/explainer-motion/TECHNIQUES.md` and `PRIOR_ART.md`:

| Token | Value | Purpose |
|---|---|---|
| `SpringPlan::PANEL` / `Feel::PANEL` | `0.6 s`, bounce `0.12` | Rigid panel body settling into place (`StageActor::settle_in`, `window::settle_in`) |
| `SpringPlan::CONTENT` / `Feel::CONTENT` | `0.36 s`, bounce `0.0` | Ink following its container (`CONTENT_LAG = 65 ms` behind), or overlay opacity entrance |
| `SpringPlan::ENTER` / `Feel::ENTER` | `0.45 s`, bounce `0.0` | Spatial entrance and persistent layout room opening (caption/text rise, tree fold/scroll, code line motion, chat message slot) |
| `SpringPlan::EXIT` / `Feel::EXIT` | `0.24 s`, bounce `0.0` | Clean exit fade in place — simpler and faster than entry |
| `SpringPlan::MOVE` / `Feel::MOVE` | `0.6 s`, bounce `0.0` | Point-to-point spatial translation and anchor weight glide (`.with_thresholds(1e-5, 1e-5)`) |
| `SpringPlan::SNAP` / `Feel::SNAP` | `0.3 s`, bounce `0.0` | Quick state change, focus shift, or status cross-fade |
| `SpringPlan::POP` / `Feel::POP` | `0.32 s`, bounce `0.18` | Small floating UI pop (hover card, reaction pill, delta chip) |
| `SpringPlan::CAMERA` / `Feel::CAMERA` | `1.6 s`, bounce `0.0` | Camera move with weight and a natural tail |
| `SpringPlan::LIVELY` / `Feel::LIVELY` | `0.85 s`, bounce `0.2` | Hero orb landing with deliberate overshoot |
| `Ease::DRAW` | `CubicBezier([0.45, 0.0, 0.2, 1.0])` | Wire, leader, accent bar, and axis draw-on (`DRAW_SECONDS = 0.42 s`) |
| `Ease::GLIDE` | `Ease::Smootherstep` | Minimum-jerk travel between resting holds |
| `author::STAGGER` | `millis(120)` (`120 ms`) | Canonical row/card ripple gap |
| `author::CONTENT_LAG` | `millis(65)` (`65 ms`) | Lead between a rigid container and its content |
| `author::DRAW_SECONDS` | `0.42 s` | Standard leader and accent-bar draw-on duration |

---

## 3. Polish Changes & Before/After Evidence

Every before/after contact strip and short MP4 study is stored under `output/polish/`.

### 3.1 Lower Third: Full-Advance Emergence & Sequenced Exit
- **Code**: `crates/psychopomp/src/lower_third.rs`, `crates/psychopomp-render/src/render/lower_third.rs`
- **What changed**:
  - Placed the stationary clip edge flush with the accent bar's right side (`plan.origin[0] + plan.bar_width()`) instead of `12.5 px` to its right.
  - Scaled each line's slide distance to `sprite.advance + gap` instead of a fixed `size * 1.4` (`64 px`), so the full word genuinely slides out from behind the bar (`Ease::DRAW`, `120 ms` stagger) instead of clipping `"o"` off `"opencode"` in open space.
  - Sequenced `LowerThirdActor::hide` (`role` at `+0 ms`, `name` at `+60 ms` over `280 ms`, `bar` at `+300 ms` over `260 ms` on `Ease::GLIDE`) so the bar stays full-height until the text has tucked behind it.
- **Evidence**:
  - Entrance before/after: `output/polish/lower-third-show-before.jpg` vs `output/polish/lower-third-show-after.jpg` (`0.8:1.8:0.08`)
  - Exit before/after: `output/polish/lower-third-hide-before.jpg` vs `output/polish/lower-third-hide-after.jpg` (`4.8:5.45:0.05`)
  - Video clip: `output/polish/lower-third-clip.mp4` (`0.7..5.6 s`)

### 3.2 Reel Transitions: Cushioned Slide Tail, Smooth Flash Attack & Restrained Light Leak
- **Code**: `crates/psychopomp/src/plan/transition.rs`, `crates/psychopomp-render/src/render/transition/light.rs`
- **What changed**:
  - `slide_travel`: blended `dynamics::settle` with a cubic cushion (`1 - u³(1 + 3t)`, weight `0.21`) so the slide still arrives early (`position(0.2) > 0.5`) and rests with zero velocity at both ends, while retaining `3.5×` more travel across `[0.5, 1.0]` instead of stalling for the final `320 ms`.
  - `flash_intensity` & `FLASH_GAIN`: replaced the delayed cubic attack (`rise³`) with `smoothstep(t / FLASH_PEAK)` and lowered `FLASH_GAIN` from `6.0` to `3.6` so the outgoing frame blooms into the cut instead of popping in two frames.
  - `leak_strength` & `LEAK_GAIN`: replaced `sin(πt)^1.5` (infinite second derivative at endpoints) with `sin²(πt)` and lowered `LEAK_GAIN` from `2.4` to `1.35` so the warm anamorphic leak veils the cut without scorching 85% of the frame white for `500 ms`.
  - `ink_threshold`: switched from `smoothstep` to `smootherstep` for continuous acceleration at both ends.
- **Evidence**:
  - Slide before/after: `output/polish/transitions-slide-before.jpg` vs `output/polish/transitions-slide-after.jpg` (`15.5:16.5:0.08`), clip `output/polish/transitions-slide-clip.mp4`
  - Flash before/after: `output/polish/transitions-flash-before.jpg` vs `output/polish/transitions-flash-after.jpg` (`31.8:32.7:0.06`)
  - Light leak before/after: `output/polish/transitions-leak-before.jpg` vs `output/polish/transitions-leak-after.jpg` (`35.8:37.5:0.12`), clip `output/polish/transitions-flash-leak-clip.mp4`

### 3.3 Subtitles: Unbroken Minimum-Jerk Backing Morph & Snappy Swap Handoff
- **Code**: `crates/psychopomp/src/subtitles.rs`
- **What changed**:
  - Fixed `SubtitleLayout::backing` during direct page swaps (`page.swaps` / `inherited`): previously it fed already-`smoothstep`ped `leaving * 0.5` and `0.5 + entering * 0.5` into `smootherstep`, causing the backing width/height morph to stall at zero velocity (`dk/dt = 0`) at `k = 0.5`. Now `smootherstep` is evaluated on linear progress across the swap window so the backing morphs at peak velocity right through the handoff.
  - Tightened direct-swap fade-out/fade-in (`SWAP_OUT = 0.08 s`, `SWAP_IN = 0.11 s`) while keeping outgoing and incoming sentences strictly sequential so dissimilar lines never collide.
  - Removed double-easing (`smootherstep(smoothstep(t))`) on same-line highlight pill glides.
- **Evidence**:
  - Swap before/after: `output/polish/subtitles-swap-before.jpg` vs `output/polish/subtitles-swap-after.jpg` (`40.35:40.95:0.04`)
  - Video clip: `output/polish/subtitles-swap-clip.mp4` (`39.8..42.5 s`)

### 3.4 Readout, Meter & Benchmark Bars: Crisp Stationary Digits & Monotonic Springs
- **Code**: `crates/psychopomp-render/src/render/viz/readout.rs`, `crates/psychopomp/src/meter.rs`, `crates/psychopomp/src/bars.rs`
- **What changed**:
  - Matched `viz/readout.rs`'s `SMEAR_ROWS` (`0.04`) to `rolling.rs`'s `SMEAR_PER_ROW` (`0.035`) instead of `0.16` (`4×` over-blurred), and gated smear on higher places (`cell.place > 0`) so a tens or hundreds digit only smears while it is actively carrying (`(wheel - wheel.round()).abs() > 1e-4`).
  - Removed bounce from `MeterActor::set` (`0.12 -> 0.0`), `BarsActor::set` (`0.08 -> 0.0`), and `BarsActor::sort` (`0.10 -> 0.0`), and unified `BarsActor::reveal_deltas` on `SpringPlan::POP` (`0.32 s, 0.18`). Gauge and benchmark readouts now count monotonically to their target without ticking past and rolling backward.
- **Evidence**:
  - Meters before/after: `output/polish/viz-meters-before.jpg` vs `output/polish/viz-meters-after.jpg` (`13.2:22.8:0.6`), clip `output/polish/viz-meters-clip.mp4`
  - Bars before/after: `output/polish/viz-bars-before.jpg` vs `output/polish/viz-bars-after.jpg` (`23.8:33.4:0.6`), clip `output/polish/viz-bars-clip.mp4`

### 3.5 Overlays: Callout Leader Handoff, Unified Lens Springs & `hide` Defaults
- **Code**: `crates/psychopomp/src/callout.rs`, `crates/psychopomp/src/lens.rs`, `crates/psychopomp/src/ide.rs`, `crates/psychopomp/src/caption.rs`, `crates/psychopomp/src/text.rs`, `crates/psychopomp/src/video.rs`, `crates/psychopomp/src/image.rs`, `crates/psychopomp/src/footage.rs`, `crates/psychopomp/src/plot.rs`, `crates/psychopomp/src/anchor.rs`
- **What changed**:
  - Reduced `CalloutActor` `LABEL_DELAY` from `280 ms` to `180 ms` so the label rises in as the `Ease::DRAW` leader rounds the knee onto the shelf rather than leaving an empty wire pointing at nothing for `80–100 ms`.
  - Unified `LensActor`'s `move_to`, `slide`, `resize`, `magnify`, and `focus` on `SpringPlan::MOVE` (`0.6 s, 0.0` bounce) and tightened `show` to `0.5 s, 0.14` bounce so the loupe glides and reshapes between circle and capsule as one piece of glass without rim wobble.
  - Fixed `CalloutActor::hide`, `LensActor::hide`, `HoverActor::hide`, `CursorActor::hide`, and `PlotActor::stop_ride` to declare their channels at their visible resting value (`1.0`) when `hide` is called without a prior `show`.
  - Unified `caption::hide`, `TextActor::hide`, `VideoActor::hide`, `ImageActor::hide`, `FootageActor::hide`, and `LensActor::hide` on `SpringPlan::EXIT` (`0.24 s`, faster than `0.35–0.45 s` entries).
  - Replaced `Ease::CubicInOut` (mid-flight acceleration jump) with `Ease::GLIDE` (`Smootherstep`) in `CursorActor::select`, `FootageActor::drift`, and `FootageActor::treat`.
- **Evidence**:
  - Callout enter before/after: `output/polish/callouts-enter-before.jpg` vs `output/polish/callouts-enter-after.jpg` (`1.2:2.5:0.1`), clip `output/polish/callouts-enter-clip.mp4`
  - Loupe capsule before/after: `output/polish/loupe-capsule-before.jpg` vs `output/polish/loupe-capsule-after.jpg` (`3.2:6.4:0.2`), clip `output/polish/loupe-capsule-clip.mp4`

### 3.6 Text Surfaces & Tree: Zero-Bounce Chat Slot Growth & Synchronized Tree Scroll
- **Code**: `crates/psychopomp/src/chat.rs`, `crates/psychopomp/src/tree.rs`
- **What changed**:
  - Set `ChatActor` `SAY_BOUNCE = 0.0` (was `0.12`) so opening a message slot lifts the thread history monotonically without bouncing every older message up and down, reduced `REACT_BOUNCE` to `0.2`, and aligned `ChatActor::stream`'s text start with `author::CONTENT_LAG` (`65 ms`).
  - Matched `TreeActor` `SCROLL_SECONDS` and `VALUE_SECONDS` to `OPEN_SECONDS` (`0.45 s`, `SpringPlan::ENTER`) so opening a container and scrolling to reveal its block move in locked sync.
- **Evidence**:
  - Tree fold/scroll before/after: `output/polish/tree-fold-scroll-before.jpg` vs `output/polish/tree-fold-scroll-after.jpg` (`5.7:6.6:0.08`)
  - Chat before/after: `output/polish/text-surfaces-chat-before.jpg` vs `output/polish/text-surfaces-chat-after.jpg` (`20.5:34.0:0.85`)

### 3.7 Stage, CameraRig & Showroom Choreography
- **Code**: `crates/psychopomp/src/stage.rs`, `crates/psychopomp/src/stage/camera.rs`, `scenes/effects-showroom/src/lib.rs`, `scenes/camera/src/main.rs`, `scenes/stage-forms/src/main.rs`, `scenes/psychopomp-intro/src/main.rs`
- **What changed**:
  - Changed `StageActor::scan` from `Ease::Linear` to `Ease::GLIDE` (`Smootherstep`), and upgraded `StageActor::amount`, `raise`, `lower` and `CameraRig::drift`, `whip` zoom-streak, and `handheld` from cubic `Smoothstep` to quintic `Ease::GLIDE`.
  - Fixed caption overlap (`stagstage.dissolve`) and the `1.1 s` empty-frame gap between `dissolve` and `materialize` in `scenes/effects-showroom`.
  - Scheduled `queue` and `drain` `settle_in` earlier during the `whip` pan in `scenes/camera` so the destination cluster is already settling as the camera arrives.
  - Added `PANEL`/`ENTER` spatial entrances for forms and a `shock_kick` + `land` reaction on `gateway` (with prompt `cache-name` / `fill` fade) when `cache` bursts in `scenes/stage-forms`.
  - Repositioned `"ANY FRAME!"` (`y = 276`, `size = 132`) and `"ANY ORDER!"` (`y = 730`, `size = 132`) in `scenes/psychopomp-intro` so neither collides with the clock chip or the hero orb.
- **Evidence**:
  - Effects replace/scan before/after: `output/polish/effects-replace-scan-before.jpg` vs `output/polish/effects-replace-scan-after.jpg` (`14.2:19.6:0.3`), clip `output/polish/effects-replace-scan-clip.mp4`
  - Camera whip before/after: `output/polish/camera-whip-before.jpg` vs `output/polish/camera-whip-after.jpg` (`21.8:22.7:0.06`), clip `output/polish/camera-whip-clip.mp4`
  - Stage forms burst before/after: `output/polish/stage-forms-burst-before.jpg` vs `output/polish/stage-forms-burst-after.jpg` (`12.4:13.6:0.08`), clip `output/polish/stage-forms-burst-clip.mp4`
  - Psychopomp intro before/after: `output/polish/psychopomp-intro-before.jpg` vs `output/polish/psychopomp-intro-after.jpg` (`0:34:1.7`)

---

## 4. Visible Changes Per Showroom & Film

| Showroom / Film | Visible Changes |
|---|---|
| `psychopomp-camera` | `queue` and `drain` settle in as the `whip` pan arrives (`22.28s..22.40s`); `drift`, `whip` streak, and `handheld` use `Ease::GLIDE` (`Smootherstep`); slate captions exit in `0.24 s` (`SpringPlan::EXIT`). |
| `psychopomp-stage-forms` | `slab`, `store`, and `cache` settle in with scale (`0.94 -> 1.0`), vertical drift (`16 px`), and deblur; `cache` burst (`12.50s`) clears `"cache"` and the dashed `fill` wire promptly and `gateway` takes a `shock_kick` and rim flash at `12.72s`. |
| `psychopomp-effects-showroom` | Outgoing captions fade in `0.15 s` before the next caption starts typing (no overlap at `15.70s`); `config v2` materializes `0.35 s` after `config v1` dissolves (no `1.1 s` empty gap); `scan` sweeps with `Ease::GLIDE` instead of `Ease::Linear`. |
| `psychopomp-transitions` | `slide` cushions through its second half (`16.06s..16.46s`); `flash` builds smoothly over `[0, FLASH_PEAK]` with `FLASH_GAIN = 3.6`; `light-leak` uses a `sin²(πt)` bell and `LEAK_GAIN = 1.35` so the frame stays readable; `ink` uses `smootherstep`. |
| `psychopomp-compare` | Unchanged (already uses `smootherstep` held wipes). |
| `psychopomp-callouts` | Callout labels rise in at `180 ms` as the leader rounds the elbow (`1.50s`, `1.90s`, `2.20s`); `hide` retracts the leader on `Ease::GLIDE` in `0.28 s`. |
| `psychopomp-anchors` | Callout labels use the `180 ms` handoff; captions, rolling numbers, text, and images exit on `SpringPlan::EXIT` (`0.24 s`). |
| `psychopomp-loupe` | `LensActor` `move_to`, `slide`, `resize`, `magnify`, and `focus` share `SpringPlan::MOVE` (`0.6 s`, zero bounce), so capsule-to-circle reshaping and anchor glides land together; `show` uses `0.5 s, 0.14` bounce and `hide` uses `SpringPlan::EXIT`. |
| `psychopomp-text-surfaces` | `LowerThird` slides `"opencode"` and `"coding agent"` across their full measured widths from the bar's right edge and waits to retract the bar until the text clears; `Chat` messages open slots without vertical bounce (`SAY_BOUNCE = 0.0`) and stream after `65 ms`. |
| `psychopomp-viz-components` | `Meter` and `Bars` readouts keep stationary higher-place digits crisp, use `SMEAR_ROWS = 0.04` (legible `upload` tenths and `cpu` digits), and spring monotonically without value/slot bounce; `Subtitles` morph their backing through page swaps without a midpoint stall. |
| `psychopomp-charts` | `PlotActor::stop_ride` and caption exits use `SpringPlan::EXIT` (`0.24 s`). |
| `psychopomp-tree` | `TreeActor` `scroll` (`0.45 s`) and `value` (`0.45 s`) match `open` (`0.45 s`), keeping the viewport bottom aligned during fold reveals. |
| `psychopomp-rolling-number` | Overlay exit uses `SpringPlan::EXIT` (`0.24 s`). |
| `psychopomp-diagnostics` | `CursorActor::select` sweeps with `Ease::GLIDE`; `Callout` `"Database provided"` uses the `180 ms` label handoff. |
| `psychopomp-footage` | `drift` and `treat` use `Ease::GLIDE` (`Smootherstep`); `hide` uses `SpringPlan::EXIT` (`0.24 s`). |
| `psychopomp-hello` | Unchanged (`hello` and `hello_dsl` remain byte-identical). |
| `scenes/2password` | Chips and footers exit on `SpringPlan::EXIT` (`0.24 s`); `2password` and `2password_dsl` remain byte-identical. |
| `scenes/psychopomp-intro` | `"ANY FRAME!"` (`y = 276`, `size = 132`) and `"ANY ORDER!"` (`y = 730`, `size = 132`) clear both the clock chip and the hero orb silhouette; callouts use the `180 ms` label handoff. |
| `scenes/pr-walkthrough`, `pr-50231`, `config-migration`, `opencode-jr-architecture` | Regenerated `.reel.json` plans with `SpringPlan::EXIT` (`0.24 s`) caption exits and `180 ms` callout label delay. |
