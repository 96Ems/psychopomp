# Psychopomp Authoring Language Design (`DSL.md`)

> *"First define what the things **mean** (their denotation); then choose the combinators whose meanings compose; then check the algebraic laws they obey."*
> — Conal Elliott & Simon Peyton Jones

Psychopomp's renderer has a crisp semantic core: a **Scene Plan** is a declarative collection of actors and tracks, and rendering is a pure function `Time → Pixels` with temporal shutter accumulation.

By contrast, the **authoring surface** in `scenes/*` has grown ad hoc. Authors thread raw `u64` nanosecond timestamps through mutable `PlanBuilder` calls, write `base + i * gap` loops 35+ times, clamp narration cues with `.max(arrival + seconds(0.42))` 25+ times, subtract flight times by hand (`phrase - seconds(0.8)`) 12+ times, pass raw `(duration, bounce)` pairs to every spring, and compute label coordinates with manual pixel arithmetic (`[VAULT[0], 704.0, 0.0]`).

This document designs the composable authoring language (`psychopomp::score` and `psychopomp::layout`) that ties the library together while compiling 100% down to the existing `PlanBuilder` and versioned `ScenePlan` JSON.

---

## 1. Why the Old Relative `Animation` Algebra Failed

Commit `20fc32c` removed an earlier `Animation` / `Composition` tree (`Sequence`, `Parallel`, `Delay`, `Hold`). Before designing its replacement, we must be honest about why that first attempt died while `PlanBuilder` survived:

1. **It bypassed Scene Plans.** `Composition` compiled directly into in-memory renderer structs rather than emitting a `ScenePlan` JSON artifact.
2. **It conflated *physical settling time* with *choreographic span*.** In the old `Animation::Spring`, a spring's sequential duration was `profile.advance_time()`—the ~1.1 s until position and velocity fall below `0.001`. In real motion design, the next beat almost never waits for asymptotic rest: `settle_in` is ready to wire after `500 ms` while its 0.6 s spring is still settling; `send` completes on *arrival* while its trail cools for `1.74 s`; a camera move (`to("camera.x", ...)`) is an impulse (`0 ns` choreographic wait) that glides in the background while the action continues.
3. **It accumulated `f32` seconds instead of exact `u64` nanoseconds.** Repeated `f32` additions (`0.1_f32 + 0.2_f32`) drifted from `u64` media timestamps, breaking associativity and requiring special float-widening workarounds in tests.
4. **It was a closed ADT coupled to one recipe.** `Composition` had hardcoded enum variants (`Animate`, `Annotate`, `Task`, `Play`) and could not be extended by `StageActor`, `SequenceActor`, `TreeActor`, `PlotActor`, or user functions.
5. **It was purely relative from `t = 0`.** Narrated explainers (`2password`, `pr-walkthrough`) are **hybrid**: beats are anchored to spoken phrases (`p("asks")`), chained causally (`settle_in` → `connect` → `send` → `land`), joined by causal guards (`f("injected").max(contact + 340ms)`), or back-timed to land *on* a word (`p("approve") - flight`). A tree that only knows `t = 0` cannot express narration-driven film timing.

Any new DSL must solve all five problems directly.

---

## 2. Denotational Semantics

### 2.1 Semantic Domains

Let:

- $\mathbb{T} = \mathbb{N}_{64}$ be **Plan Time** in integer nanoseconds (`u64`), with $1\text{ s} = 10^9\text{ ns}$.
- $\mathbb{R}_{\ge 0}$ be continuous **Sample Time** in seconds (`f64`).
- $\mathsf{MotionState} = \mathbb{R}_{32} \times \mathbb{R}_{32}$ be `(position, velocity)`.

#### Denotation of a Continuous Channel

$$\llbracket \mathsf{Channel} \rrbracket : \mathbb{R}_{\ge 0} \to \mathsf{MotionState}$$

A channel's meaning is a pure function of sample time, determined by an initial scalar $v_0$ and a finite sequence of timestamped events $e_1, \dots, e_k$ ordered by $(t_i, i)$:

$$\mathsf{state}(t) = \mathsf{segment}_m.\mathsf{sample}(t - t_m) \quad \text{where } m = \max \{ i \mid t_i \le t \}$$

Crucially, when event $e_i$ is a `Spring(target, profile)` at time $t_i$, its segment is initialized with the **sampled position and velocity** of the preceding trajectory at $t_i$:

$$\mathsf{initial}_i = \mathsf{state}(t_i^-)$$

#### Denotation of a `Span` (Choreographic Interval)

$$\mathsf{Span} = \{ (\mathit{start}, \mathit{end}) \in \mathbb{T} \times \mathbb{T} \mid \mathit{start} \le \mathit{end} \}$$

A `Span` is the **choreographic interval** occupied by a beat:

- `span.start`: when the beat was cued.
- `span.end`: when the beat's primary action completes (e.g. when a panel is ready, a wire makes contact, a packet lands, or a label finishes typing).
- `span.duration() = span.end - span.start`.

An **impulse** (such as flashing a card, starting a background camera glide, or firing a sound effect) has $\mathit{end} = \mathit{start}$ ($\mathit{duration} = 0$), even though the physical spring, decay curve, or audio clip continues after $\mathit{end}$.

#### Denotation of a `Beat`

$$\llbracket \mathsf{Beat} \rrbracket : \mathbb{T} \to (\mathsf{Span}, \mathsf{Writes})$$

Given a cue time $t_0 \in \mathbb{T}$ on the plan clock, a `Beat`:
1. Computes the occupied choreographic interval $\mathsf{Span}(t_0, t_{\text{end}})$.
2. Emits a finite sequence of plan writes $\mathsf{Writes}$ (channel declarations, continuous `Set`/`Spring`/`Ease` events, state events, media placements, cues) into the `PlanBuilder`.

Because Rust's actor handles (`StageActor`, `CaptionActor`, `PlotActor`, …) borrow `&mut PlanBuilder`, a `Beat` is represented in Rust as any value or closure implementing:

```ts
pub trait Beat<Ctx> {
    fn play(self, ctx: &mut Ctx, at: u64) -> Span;
}
```

And every function that already writes to `&mut PlanBuilder` at `at: u64` and returns `()` (impulse) or `u64` (end time) is **automatically** a `Beat`!

---

## 3. The Combinator Algebra & Its Laws

### 3.1 Core Combinators

| Combinator | Rust Syntax | Denotation $\llbracket \cdot \rrbracket(t)$ | Intuitive Meaning |
| --- | --- | --- | --- |
| **Empty** | `score::empty()` | $((t, t), \emptyset)$ | Do nothing; advance 0 ns |
| **Hold** | `score::hold(d)` | $((t, t + d), \emptyset)$ | Wait $d$ ns without writing |
| **Impulse** | `score::impulse(\|ctx, t\| ...)` | $((t, t), w(t))$ | Emit writes at $t$; advance 0 ns |
| **Action** | `score::action(\|ctx, t\| end)` | $((t, \mathit{end}), w(t))$ | Emit writes at $t$; advance to $\mathit{end}$ |
| **Sequence** | `a.then(b)` / `chain!(a, b, c)` | Let $(s_1, w_1) = \llbracket a \rrbracket(t)$, $(s_2, w_2) = \llbracket b \rrbracket(s_1.\mathit{end})$ in $((t, s_2.\mathit{end}), w_1 \mathbin{+\!+} w_2)$ | Play $a$, then play $b$ at $a$'s end |
| **Parallel (All)** | `a.also(b)` / `all!(a, b, c)` | Let $(s_1, w_1) = \llbracket a \rrbracket(t)$, $(s_2, w_2) = \llbracket b \rrbracket(t)$ in $((t, \max(s_1.\mathit{end}, s_2.\mathit{end})), w_1 \mathbin{+\!+} w_2)$ | Start $a$ and $b$ together at $t$; end when both finish |
| **Accompany** | `a.with(b)` | Let $(s_1, w_1) = \llbracket a \rrbracket(t)$, $(s_2, w_2) = \llbracket b \rrbracket(t)$ in $(s_1, w_1 \mathbin{+\!+} w_2)$ | Play subordinate action $b$ at $a$'s start; keep $a$'s span |
| **On End** | `a.on_end(b)` | Let $(s_1, w_1) = \llbracket a \rrbracket(t)$, $(s_2, w_2) = \llbracket b \rrbracket(s_1.\mathit{end})$ in $(s_1, w_1 \mathbin{+\!+} w_2)$ | Trigger $b$ at $a$'s end (e.g. sound/flash on arrival); keep $a$'s end unless $b$ is chained via `.then` |
| **Delay** | `a.after(d)` | Let $(s, w) = \llbracket a \rrbracket(t + d)$ in $((t, s.\mathit{end}), w)$ | Wait $d$ ns after $t$, then play $a$ |
| **Lead / Pre-roll** | `a.early(d)` | Let $t' = t \ominus d$, $(s, w) = \llbracket a \rrbracket(t')$ in $((t', s.\mathit{end}), w)$ | Start $d$ ns *before* $t$ (clamped at 0) |
| **Stagger** | `stagger(gap, items, f)` | $\mathsf{all}_{i=0}^{n-1} f(x_i).\mathsf{after}(i \cdot \mathit{gap})$ | Ripple $f(x_i)$ spaced by $\mathit{gap}$ |

### 3.2 Algebraic Laws

Because timestamps are exact `u64` nanoseconds (not `f32`), these laws hold **to the nanosecond**:

1. **Sequence Monoid (Associativity & Identity)**
   ```ts
   a.then(b).then(c)  ===  a.then(b.then(c))
   empty().then(a)    ===  a  ===  a.then(empty())
   ```
   Both the returned `Span` and the exact `Vec<TrackEventPlan>` per channel are identical.

2. **Hold Fusion & Delay Isomorphism**
   ```ts
   hold(d1).then(hold(d2))  ===  hold(d1 + d2)
   hold(d).then(a)          ===  a.after(d)
   a.after(0)               ===  a
   a.after(d1).after(d2)    ===  a.after(d1 + d2)
   ```

3. **Sequence-Delay Left Shift**
   ```ts
   a.then(b).after(d)  ===  a.after(d).then(b)
   ```
   Proof: $\llbracket a.\mathsf{then}(b).\mathsf{after}(d) \rrbracket(t)$ runs $a$ at $t + d$ yielding $s_1$, then runs $b$ at $s_1.\mathit{end}$, which is identical to running $a.\mathsf{after}(d)$ at $t$ (ending at $s_1.\mathit{end}$) followed by $b$.

4. **Parallel Monoid (Associativity & Identity)**
   ```ts
   a.also(b).also(c)  ===  a.also(b.also(c))
   empty().also(a)    ===  a  ===  a.also(empty())
   ```

5. **Delay Distributes over Parallel**
   ```ts
   a.also(b).after(d)  ===  a.after(d).also(b.after(d))
   ```

6. **Impulse Absorption in Sequence**
   If $p$ is an impulse ($\mathit{duration}(p) = 0$):
   ```ts
   p.then(a)  ===  p.also(a)
   ```
   Because $p$ advances the cursor by $0\text{ ns}$, playing $p$ then $a$ starts $a$ at the exact same timestamp $t$ and produces the exact same writes and span as `p.also(a)`.

7. **Stagger Decomposition**
   ```ts
   stagger(gap, [x0, x1, x2], f)
     === f(x0).also(f(x1).after(gap)).also(f(x2).after(2 * gap))
   ```

### 3.3 Spring Retargeting & Parallel Commutativity

What happens when two concurrent beats write to the **same continuous channel**?

In Psychopomp (`Timeline::compile_events` in `crates/psychopomp/src/timeline.rs`), events on a channel are sorted by `(at_nanos, source_index)` before compiling segments:

- **Distinct timestamps ($t_1 \neq t_2$): Parallel composition is strictly commutative on compiled trajectories.**
  ```ts
  // For any two channel writes w1 at t1 and w2 at t2 with t1 != t2:
  Timeline(w1.also(w2))  ===  Timeline(w2.also(w1))
  ```
  If $w_1$ springs `camera.x` toward `100.0` at $t = 0$ and $w_2$ springs `camera.x` toward `-40.0` at $t = 300\text{ ms}$, the compiled `Timeline` sorts $t = 0$ before $t = 300\text{ ms}$ regardless of whether the author wrote `w1.also(w2)` or `w2.also(w1)`. At $t = 300\text{ ms}$, $w_2$ samples $w_1$'s live `(position, velocity)` and smoothly redirects.

- **Equal timestamps ($t_1 = t_2$) on the same channel: Ordered state transformation.**
  When two events share the exact same nanosecond on the same channel, source order matters in exactly one legitimate idiom: **`Set(from)` followed immediately by `Spring(to)` or `Ease(to)`** (used in `hit`, `settle_in`, `clock_for`, `twang`).
  - Within a single beat or `then` chain, `set(t, from).then(spring(t, to))` is deterministic and intentional.
  - Across two independent parallel branches `a.also(b)`, writing two *competing* targets (e.g. `Spring(10.0)` and `Spring(20.0)`) to the same channel at the exact same nanosecond $t$ is almost always a choreography bug.
  - **Rule**: `PlanBuilder` preserves source order deterministically (so existing plans stay byte-identical), while `score::find_write_conflicts(&plan)` detects any channel with two same-timestamp `Spring`/`Ease` writes or out-of-order `Set`-after-`Spring` at the same timestamp and reports the channel and time.

---

## 4. Hybrid Time: Bridging Narration Anchors & Relative Beats

In narrated films (`scenes/2password`, `scenes/pr-walkthrough`), time is neither purely absolute nor purely relative—it is a **DAG of speech cues and causal chains**:

```ts
// What authors write today in scenes/2password/src/main.rs:
let process_ready = s.settle_in(sc, "process", f("runs"));
let inject_contact = s.connect(sc, "inject", process_ready, 0.4);
sound(sc, "connect-inject", TICK, inject_contact, -19.0);
let secrets = s.send(
    sc,
    "secrets",
    f("injected").max(inject_contact + seconds(0.34)),
    0.5,
);
s.land(sc, "process", secrets);
s.to(sc, "process.status", secrets, 1.0, 0.3);
s.to(sc, "layer.status", secrets, 3.0, 0.3);
sound(sc, "secrets", CONFIRM, secrets, -12.0);
```

Notice the three recurring temporal relationships:
1. **Sequential chain from a cue**: at `f("runs")`, `settle_in("process")`, then `connect("inject", 0.4)`, then on contact play `TICK`.
2. **Causal join (`not_before`)**: cue at `f("injected")`, *but not before* `inject_contact + 340 ms` (so the packet never flies down an undrawn wire!).
3. **Arrival fan-out**: when `send("secrets", 0.5)` lands, simultaneously `land("process")`, update statuses, and play `CONFIRM`.

With `Span` as the return value of running a beat on a `Score` (and `Span` implementing time-anchor methods `.end()`, `.after(d)`, `.not_before(t)`), this becomes:

```ts
let inject = sc.at(
    f("runs"),
    s.settle_in("process")
        .then(s.connect("inject", 0.4))
        .on_end(sound("connect-inject", TICK, -19.0)),
);

sc.at(
    f("injected").not_before(inject.after(millis(340))),
    s.send("secrets", 0.5).then(all![
        s.land("process"),
        s.to("process.status", 1.0, 0.3),
        s.to("layer.status", 3.0, 0.3),
        sound("secrets", CONFIRM, -12.0),
    ]),
);
```

Look at what disappeared:
- Zero temporary `u64` variables (`process_ready`, `inject_contact`, `secrets`) threaded by hand across 15 lines.
- No repeated `&mut scene` (`sc`) passed into every single method call.
- `inject.after(millis(340))` reads in plain English ("340 ms after `inject` completes").
- `s.send("secrets", 0.5).then(...)` automatically runs the arrival reactions at the exact nanosecond the packet lands!

---

## 5. Spatial Composition (`psychopomp::layout`)

Just as authors hand-computed `base + i * gap` in time, they hand-compute coordinates in space:

```ts
// Today in scenes/2password/src/main.rs:
const VAULT: [f32; 3] = [1650.0, 540.0, 0.0];
// ...
StageElement::Orb { id: "vault".into(), at: VAULT, radius: 118.0, .. },
label("vault-name", [VAULT[0], 704.0, 0.0], 24.0, ...), // where did 704 come from?! (540 + 118 + 46)

// Today in scenes/pr-walkthrough/src/flagship.rs:
for (i, ..) in rows.iter().enumerate() {
    let x = 360.0 + i as f32 * 400.0;
    ...
}
```

Borrowing from Haskell's `diagrams` library (envelopes/bounding regions, relative placement, and distribution) while respecting `PLAN.md` (*"Layout produces targets, not animation"*), `psychopomp::layout` provides pure, GPU-free 2D/2.5D geometry values:

```ts
/// A 2.5D bounding box on the Stage canvas (depth-0 pixel coordinates + z plane).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub center: [f32; 3],
    pub size: [f32; 2],
}
```

### Spatial Combinators

- **From elements**:
  - `Placement::card(at, size)` → box of `size` at `at`
  - `Placement::orb(at, radius)` → square box `[2*r, 2*r]` at `at`
  - `StagePlan::placement(id)` → looks up any positioned `StageElement`'s `Placement`!
- **Relative anchors (Envelope edges + gap)**:
  - `p.below(gap)` → `[p.center.x, p.bottom() + gap, p.center.z]`
  - `p.above(gap)` → `[p.center.x, p.top() - gap, p.center.z]`
  - `p.right_of(gap, width)` → `[p.right() + gap + width * 0.5, p.center.y, p.center.z]`
  - `p.left_of(gap, width)` → `[p.left() - gap - width * 0.5, p.center.y, p.center.z]`
  - `p.align_left(inset)` → `p.left() + inset` (replaces `AGENT[0] - CARD[0] * 0.5 + 4.0`)
- **Distribution**:
  - `layout::row(center, pitch, count)` / `layout::column(center, pitch, count)`
  - `layout::spread_x(left, right, y, count)`

### Spatial Laws

1. **Opposite Edge Round-trip**: `p.right_of(gap, w).left_of(gap, p.width) == p.center`
2. **Row Centroid Invariant**: For any `row(center, pitch, n)` with $n \ge 1$, the arithmetic mean of the $n$ points equals `center`.
3. **Translation Equivariance**: `p.translate(delta).below(gap) == p.below(gap) + delta`.

---

## 6. Actor & Identity Coherence

Across the codebase, nine actor handles (`StageActor`, `SequenceActor`, `CaptionActor`, `RollingNumberActor`, `TreeActor`, `PlotActor`, `LanesActor`, `CalloutActor`, `VideoActor`) evolved separately. The DSL unifies them around four rules:

1. **Recipe owns channel defaults.**
   Previously, `StageActor::to` declared any uninitialized channel at `0.0`, even though `Stage` defaults `opacity`, `scale`, `content`, `spin`, `typed`, `sweep`, and `post.exposure` to `1.0`, and `burst`, `mark`, `spinner`, `release`, `post.rewind` to `-1.0`. Fading an element out with `stage.to("link.opacity", at, 0.0, 0.5)` silently started its opacity at `0.0`! Every recipe handle must resolve undeclared channels via its recipe default table (`StagePlan::channel_default`), as landed in the `authoring` branch.
2. **Uniform handle accessors.**
   Every actor handle exposes `pub fn actor(&self) -> &ActorHandle` and `pub fn id(&self) -> &str`.
3. **Uniform visibility vocabulary.**
   Every visual overlay and Stage element supports `show` (enter/fade to visible) and `hide` (fade to `0.0` from its resting `1.0` default).
4. **Named Motion Profiles (`SpringPlan` feels).**
   Instead of raw `(0.6, 0.12)` or `(1.6, 0.0)` magic floats at every call site, `SpringPlan` provides calibrated constants (`PANEL`, `CONTENT`, `SNAP`, `CAMERA`, `LIVELY`) from `explainer-motion/TECHNIQUES.md` alongside custom `SpringPlan::visual(duration, bounce)`.

---

## 7. Before & After Call Sites

### Call Site 1: `scenes/hello/src/main.rs`

**Before (manual timestamp threading + `&mut scene` everywhere):**
```ts
let mut scene = PlanBuilder::new("hello", 4 * SECOND);
let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;
let ready = stage.settle_in(&mut scene, "client", 0); // the card drifts into place
let wired = stage.connect(&mut scene, "link", ready, 0.6); // the wire draws on
let landed = stage.send(&mut scene, "hello", wired + SECOND / 2, 0.8); // a packet flies
stage.land(&mut scene, "server", landed); // the orb lights up
stage.jolt(&mut scene, landed, [1.0, 0.0], 0.6); // and the camera takes the hit
```

**After (composable Beat pipeline on `Score`):**
```ts
let mut scene = PlanBuilder::new("hello", 4 * SECOND);
let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;
stage.score(&mut scene).at(
    0,
    stage::settle_in("client")
        .then(stage::connect("link", 0.6))
        .then_after(SECOND / 2, stage::send("hello", 0.8))
        .then(stage::land("server").also(stage::jolt([1.0, 0.0], 0.6))),
);
```
*Emits byte-identical `target/hello.json`.* Every step of the story—settle, connect, wait half a second, send, land + jolt—reads top-to-bottom as a single composable value.

---

### Call Site 2: Staggered Glitch & Damage (`scenes/2password/src/main.rs`)

**Before:**
```ts
let stuck = p("stuck");
s.to(sc, "agent.status", stuck, 2.0, 0.2);
for (index, card) in [
    "op", "agent", "prompt-1", "prompt-2", "prompt-3", "prompt-4",
]
.iter()
.enumerate()
{
    let at = stuck + seconds(index as f64 * 0.035);
    glitch(s, sc, card, at, [7.0, 9.0, 8.0]);
    glitch(s, sc, card, at + seconds(0.32), [9.0, 6.0, 7.0]);
    s.set(sc, &format!("{card}.damage"), at, 1.0);
}
s.jolt(sc, stuck, [-0.4, 1.0], 1.0);
s.hit(sc, "post.chroma", stuck, 0.2, 0.0);
s.hit(sc, "post.bloom", stuck, 0.35, 0.18);
```

**After:**
```ts
let cards = ["op", "agent", "prompt-1", "prompt-2", "prompt-3", "prompt-4"];
s.score(sc).at(
    p("stuck"),
    all![
        stage::to("agent.status", 2.0, 0.2),
        score::stagger(seconds(0.035), cards, |card| {
            all![
                stage::glitch(card, [7.0, 9.0, 8.0]),
                stage::glitch(card, [9.0, 6.0, 7.0]).after(seconds(0.32)),
                stage::set(format!("{card}.damage"), 1.0),
            ]
        }),
        stage::jolt([-0.4, 1.0], 1.0),
        stage::hit("post.chroma", 0.2, 0.0),
        stage::hit("post.bloom", 0.35, 0.18),
    ],
);
```
Look at the inner beat: `stage::glitch(card, [9.0, 6.0, 7.0]).after(seconds(0.32))` is offset *relative to each card's own staggered start*, and `score::stagger` distributes the `35 ms` ripple automatically via Law 5 (`Delay Distributes over Parallel`).

---

### Call Site 3: Phrase-Anchored Request/Reply Chain (`scenes/2password/src/main.rs`)

**Before:**
```ts
let everything = l("everything");
s.to(sc, "agent.status", everything, 4.0, 0.3);
let find = s.send(sc, "find", everything - seconds(0.2), 0.6);
sound(sc, "find", SEND, everything - seconds(0.2), -11.0);
s.land(sc, "layer", find);
s.to(sc, "layer.status", find, 1.0, 0.3);
let lookup = s.send(
    sc,
    "lookup",
    l("just once")
        .saturating_sub(seconds(0.35))
        .max(find + seconds(0.42)),
    0.55,
);
sound(sc, "lookup", SEND, lookup - seconds(0.55), -11.0);
s.land(sc, "op", lookup);
let fetch = s.send(sc, "fetch", lookup + seconds(0.42), 0.45);
s.land(sc, "vault", fetch);
```

**After:**
```ts
let mut sc = s.score(scene);
let find = sc.at(
    l("everything"),
    stage::to("agent.status", 4.0, 0.3).also(
        stage::send("find", 0.6)
            .with(sfx("find", SEND, -11.0))
            .early(seconds(0.2))
            .then(stage::land("layer").also(stage::to("layer.status", 1.0, 0.3))),
    ),
);
sc.at(
    l("just once").early(seconds(0.35)).not_before(find.after(seconds(0.42))),
    stage::send("lookup", 0.55)
        .with(sfx("lookup", SEND, -11.0))
        .then(stage::land("op"))
        .then_after(seconds(0.42), stage::send("fetch", 0.45))
        .then(stage::land("vault")),
);
```
Notice how `.with(sfx("lookup", SEND, -11.0))` attaches the sound to the *start* of `send("lookup", 0.55)`—eliminating the bug-prone `lookup - seconds(0.55)` where the author had to subtract the flight duration from the arrival time just to recover the launch time!

---

## 8. Explicit Decision Points

### Decision Point 1: How Beats bind to Actors & `PlanBuilder`

- **Option A (Context-parameterized `Beat<Ctx>` trait + free/associated constructors):**
  `Beat<Ctx>` is implemented for closures `FnOnce(&mut Ctx, u64) -> Span` and combinator structs (`Then`, `Also`, `With`, `OnEnd`, `After`, `Early`, `Stagger`). `StageScore<'a>` holds `(&'a mut StageActor, &'a mut PlanBuilder)` as its `Ctx`, while scene-wide scores can use `&mut PlanBuilder` or a user tuple `(&mut StageActor, &mut CaptionActor, &mut PlanBuilder)`.
- **Option B (Rc/RefCell interior mutability on `PlanBuilder` and Actor handles):**
  Wrap `PlanBuilder` in `Rc<RefCell<PlanBuilder>>` so every actor clones a shared reference and methods like `stage.send("hello", 0.8)` need no context argument at play time.
- **Recommendation: Option A.**
  Option A is zero-cost, idiomatic Rust, needs no `Rc<RefCell<...>>` runtime borrow checking, works seamlessly with existing `&mut PlanBuilder` and `&mut StageActor` signatures, and lets any closure `|s, at| { ... }` participate as a first-class `Beat` without cloning handles.

### Decision Point 2: Operator Sugar (`>>`, `&`) vs. Fluent Methods (`.then()`, `.also()`)

- **Option A (Fluent methods + variadic macros `chain![...]` / `all![...]`):**
  Use `.then(b)`, `.then_after(d, b)`, `.also(b)`, `.with(b)`, `.on_end(b)`, `.after(d)`, `.early(d)` for binary composition and `chain![a, b, c]` / `all![a, b, c]` for flat lists.
- **Option B (Bitwise operator overloading `a >> b` for sequence, `a & b` for parallel):**
  Implement `std::ops::Shr` and `std::ops::BitAnd` on `Beat` wrappers.
- **Recommendation: Option A.**
  In Rust, operator overloading on generic traits (`Beat<Ctx>`) requires wrapping every leaf in a concrete newtype struct (due to orphan rules), has confusing precedence between `>>`, `&`, and `+`, and formats poorly under `rustfmt`. Fluent methods (`.then()`, `.also()`, `.with()`, `.after()`) plus `chain!` / `all!` format cleanly under `rustfmt`, read self-evidently at call sites, and need zero wrapper boilerplate on closures.

### Decision Point 3: Same-Timestamp Parallel Writes to the Same Channel

- **Option A (Preserve source order + explicit diagnostic helper):**
  `a.also(b)` emits `a`'s writes followed by `b`'s writes. If they touch different channels or different timestamps, order is irrelevant (`Timeline` sorts by `at_nanos`). If they touch the same channel at the exact same nanosecond, source order is preserved (matching `PlanBuilder` today), and `score::lint_conflicts(&plan)` can be called in tests/validation to flag accidental same-time competing writes.
- **Option B (Hard panic / `Result::Err` during `Beat::play` on any same-time same-channel write):**
  Track a `(ChannelId, u64)` set inside `Score` and panic if two parallel branches write to the same channel at the same nanosecond.
- **Recommendation: Option A.**
  Existing scenes (and `StageActor::settle_in` / `hit` / `clock_for`) legitimately emit `Set(at)` followed by `Spring(at)` or `Ease(at)` at the same timestamp, and some scenes reset a channel with `set` and start a new motion at the same cue. Preserving deterministic source order guarantees 100% byte-identical `ScenePlan` output while allowing targeted conflict linting.

### Decision Point 4: Module Naming (`psychopomp::score` vs `psychopomp::dsl`)

- **Option A (`psychopomp::score` + `psychopomp::layout`):**
  Name the temporal composition module `score` (`Score`, `Beat`, `Span`, `CueTime`, `chain!`, `all!`, `stagger`) and the spatial module `layout` (`Placement`, `row`, `column`, `spread_x`).
- **Option B (`psychopomp::dsl`):**
  Reuse the deleted `dsl.rs` module name.
- **Recommendation: Option A (`score` + `layout`).**
  "DSL" describes *how* an API is implemented, not *what domain concept* it represents (`CONTEXT.md`). In motion graphics and film, a timed arrangement of beats on a clock is a **Score**, and spatial arrangement on the Stage is **Layout**.

---

## 9. Honesty with Psychopomp's Non-Abstractions

This design strictly upholds `AGENTS.md` and `ARCHITECTURE.md`:

- **No Scene Plan format change.** `Score` and `Beat` compile directly into existing `PlanBuilder` calls (`set`, `spring`, `ease`, `media`, `cue`). The emitted JSON is versioned `ScenePlan` v2, byte-for-byte identical to hand-written `PlanBuilder` code.
- **No runtime scene graph or callbacks.** `Beat::play` executes once at plan-emission time in the lightweight `psychopomp` crate. `psychopomp-render` is untouched and continues to sample compiled `PropertyTrack`s as a pure function of time.
- **Ordinary Rust stays in charge.** Loops, conditionals, helper functions, and arithmetic remain ordinary Rust—now operating over first-class composable `Beat`, `Span`, and `Placement` values instead of raw `u64` and `[f32; 3]` literals.
