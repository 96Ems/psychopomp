# Psychopomp Authoring Language Design (`DSL.md`)

> *"First define what the things **mean** (their denotation); then choose the combinators whose meanings compose; then check the algebraic laws they obey."*
> — Conal Elliott & Simon Peyton Jones

Psychopomp's renderer has a crisp semantic core: a **Scene Plan** is a declarative collection of actors and tracks, and rendering is a pure function `Time → Pixels` with temporal shutter accumulation.

By contrast, the **authoring surface** in `scenes/*` grew ad hoc. Authors threaded raw `u64` nanosecond timestamps through mutable `PlanBuilder` and `&mut *Actor` calls, wrote `base + i * gap` loops 35+ times, clamped narration cues with `.max(arrival + seconds(0.42))` 25+ times, subtracted flight times by hand (`phrase - seconds(0.8)`) 12+ times, passed raw `(duration, bounce)` pairs to every spring, and computed label coordinates with manual pixel arithmetic (`[VAULT[0], 704.0, 0.0]`).

This document designs the composable authoring language (`psychopomp::score` and `psychopomp::layout`) that ties the library together around one structural rule:

> **The `PlanBuilder` is the only mutable thing; actor handles are values.**

---

## 1. Why the Old Relative `Animation` Algebra Failed

Commit `20fc32c` removed an earlier `Animation` / `Composition` tree (`Sequence`, `Parallel`, `Delay`, `Hold`). Before designing its replacement, we must be honest about why that first attempt died while `PlanBuilder` survived:

1. **It bypassed Scene Plans.** `Composition` compiled directly into in-memory renderer structs rather than emitting a `ScenePlan` JSON artifact.
2. **It conflated *physical settling time* with *choreographic span*.** In the old `Animation::Spring`, a spring's sequential duration was `profile.advance_time()`—the ~1.1 s until position and velocity fall below `0.001`. In real motion design, the next beat almost never waits for asymptotic rest: `settle_in` is ready to wire after `500 ms` while its 0.6 s spring is still settling; `send` completes on *arrival* while its trail cools for `1.74 s`; a camera move (`to("camera.x", ...)`) is an impulse (`0 ns` choreographic wait) that glides in the background while the action continues.
3. **It accumulated `f32` seconds instead of exact `u64` nanoseconds.** Repeated `f32` additions (`0.1_f32 + 0.2_f32`) drifted from `u64` media timestamps, breaking associativity and requiring special float-widening workarounds in tests.
4. **It was a closed ADT coupled to one recipe.** `Composition` had hardcoded enum variants (`Animate`, `Annotate`, `Task`, `Play`) and could not be extended by `Stage`, `Sequence`, `Tree`, `Plot`, `Callout`, `Caption`, or user functions.
5. **It was purely relative from `t = 0`.** Narrated explainers (`2password`, `pr-walkthrough`) are **hybrid**: beats are anchored to spoken phrases (`p("asks")`), chained causally (`settle_in` → `connect` → `send` → `land`), joined by causal guards (`f("injected").max(contact + 340ms)`), or back-timed to land *on* a word (`p("approve") - flight`). A tree that only knows `t = 0` cannot express narration-driven film timing.

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
- `span.end`: when the beat's primary action completes (when a panel is ready, a wire makes contact, a packet lands, a roll completes, or a caption finishes typing).
- `span.duration() = span.end - span.start`.

An **impulse** (such as flashing a card, starting a background camera glide, showing/hiding a chip, or firing a sound effect) has $\mathit{end} = \mathit{start}$ ($\mathit{duration} = 0$), even though the physical spring, decay curve, or audio clip continues after $\mathit{end}$.

#### Denotation of a `Beat`

$$\llbracket \mathsf{Beat} \rrbracket : \mathbb{T} \to (\mathsf{Span}, \mathsf{Writes})$$

There is **one** mutable authoring target in Psychopomp: `PlanBuilder`. Every `Beat` plays against `&mut PlanBuilder` at a cue timestamp `at: u64`:

```ts
pub trait Beat: Sized {
    fn play(self, scene: &mut PlanBuilder, at: u64) -> Span;
}
```

Actor handles (`Stage`, `Caption`, `Callout`, `RollingNumber`, `Tree`, `Plot`, `Lanes`, `Sequence`, `Video`) are immutable `Clone` values. Their choreography methods take `&self` and return `impl Beat + '_`. Because all mutable state (actor recipe payloads, channel declarations, events, media placements, cues) lives inside `PlanBuilder`, **every actor in the library composes inside the same `Beat` tree with zero per-actor contexts or escape hatches**.

---

## 3. The Combinator Algebra & Its Laws

### 3.1 Core Combinators

| Combinator | Rust Syntax | Denotation $\llbracket \cdot \rrbracket(t)$ | Intuitive Meaning |
| --- | --- | --- | --- |
| **Empty** | `score::empty()` | $((t, t), \emptyset)$ | Do nothing; advance 0 ns |
| **Hold** | `score::hold(d)` | $((t, t + d), \emptyset)$ | Wait $d$ ns without writing |
| **Impulse** | `score::impulse(\|sc, t\| ...)` | $((t, t), w(t))$ | Emit writes at $t$; advance 0 ns |
| **Action** | `score::action(\|sc, t\| end)` | $((t, \mathit{end}), w(t))$ | Emit writes at $t$; advance to $\mathit{end}$ |
| **Sequence** | `a.then(b)` / `chain![a, b, c]` | Let $(s_1, w_1) = \llbracket a \rrbracket(t)$, $(s_2, w_2) = \llbracket b \rrbracket(s_1.\mathit{end})$ in $((t, s_2.\mathit{end}), w_1 \mathbin{+\!+} w_2)$ | Play $a$, then play $b$ at $a$'s end |
| **Parallel (All)** | `a.also(b)` / `all![a, b, c]` | Let $(s_1, w_1) = \llbracket a \rrbracket(t)$, $(s_2, w_2) = \llbracket b \rrbracket(t)$ in $((t, \max(s_1.\mathit{end}, s_2.\mathit{end})), w_1 \mathbin{+\!+} w_2)$ | Start $a$ and $b$ together at $t$; end when both finish |
| **Accompany** | `a.with(b)` | Let $(s_1, w_1) = \llbracket a \rrbracket(t)$, $(s_2, w_2) = \llbracket b \rrbracket(t)$ in $(s_1, w_1 \mathbin{+\!+} w_2)$ | Play subordinate action $b$ at $a$'s start; keep $a$'s span |
| **On End** | `a.on_end(b)` | Let $(s_1, w_1) = \llbracket a \rrbracket(t)$, $(s_2, w_2) = \llbracket b \rrbracket(s_1.\mathit{end})$ in $(s_1, w_1 \mathbin{+\!+} w_2)$ | Trigger $b$ at $a$'s end (e.g. sound/flash on contact); keep $a$'s span |
| **Delay** | `a.after(d)` | Let $(s, w) = \llbracket a \rrbracket(t + d)$ in $((t, s.\mathit{end}), w)$ | Wait $d$ ns after $t$, then play $a$ |
| **Lead / Pre-roll** | `a.early(d)` | Let $t' = t \ominus d$, $(s, w) = \llbracket a \rrbracket(t')$ in $((t', s.\mathit{end}), w)$ | Start $d$ ns *before* $t$ (clamped at 0) |
| **Stagger** | `stagger(gap, items, f)` | $\mathsf{all}_{i=0}^{n-1} f(x_i).\mathsf{after}(i \cdot \mathit{gap})$ | Ripple $f(x_i)$ spaced by $\mathit{gap}$ |
| **Each** | `each(items, f)` | $\mathsf{stagger}(0, \mathit{items}, f)$ | Fan out $f(x_i)$ simultaneously at $t$ |

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
   p.then(a)     ===  p.also(a)
   a.on_end(p)   ===  a.then(p)
   ```

7. **Stagger & Each Decomposition**
   ```ts
   stagger(gap, [x0, x1, x2], f)
     === f(x0).also(f(x1).after(gap)).also(f(x2).after(2 * gap))
   each([x0, x1], f)  ===  f(x0).also(f(x1))
   ```

### 3.3 Spring Retargeting & Parallel Commutativity

What happens when two concurrent beats write to the **same continuous channel**?

In `PlanBuilder::finish()`, events on each channel are stably sorted by `at_nanos` (`events.sort_by_key(TrackEventPlan::at_nanos)`), and `Timeline::compile_events` compiles them in `(at_nanos, source_index)` order:

- **Distinct timestamps ($t_1 \neq t_2$): Parallel composition is strictly commutative on both `ScenePlan` JSON and compiled `Timeline` trajectories.**
  ```ts
  // For any two channel writes w1 at t1 and w2 at t2 with t1 != t2:
  ScenePlan(w1.also(w2))  ===  ScenePlan(w2.also(w1))
  Timeline(w1.also(w2))   ===  Timeline(w2.also(w1))
  ```
  If $w_1$ springs `camera.x` toward `100.0` at $t = 0$ and $w_2$ springs `camera.x` toward `-40.0` at $t = 300\text{ ms}$, the compiled track sorts $t = 0$ before $t = 300\text{ ms}$ regardless of whether the author wrote `w1.also(w2)` or `w2.also(w1)`. At $t = 300\text{ ms}$, $w_2$ samples $w_1$'s live `(position, velocity)` and smoothly redirects.

- **Equal timestamps ($t_1 = t_2$) on the same channel: Ordered state transformation.**
  When two events share the exact same nanosecond on the same channel, source order matters in one legitimate idiom: **`Set(from)` followed immediately by `Spring(to)` or `Ease(to)`** (used in `hit`, `settle_in`, `clock_for`).
  - `PlanBuilder::finish()` uses a stable sort so same-timestamp `Set`-then-`Spring` pairs preserve their authored order.
  - `score::find_write_conflicts(&plan)` detects any channel with two same-timestamp `Spring`/`Ease` writes or a `Set` after a `Spring` at the same timestamp and reports the channel and time.

---

## 4. Actor Handles Are Values; `PlanBuilder` Owns All Mutable State

Previously, two actor handles cached mutable state outside `PlanBuilder`:
- `RollingNumberActor` held `plan: RollingNumberPlan` and mutated `self.plan.rolls` in `roll(&mut self, ...)`.
- `TreeActor` held `plan: TreePlan`, `open: Vec<bool>`, and `scroll: f32`, mutating them in `open`, `close`, `set`, `scroll_to`, and `reveal`.

Meanwhile, `PlanBuilder` *already* owned the serialized `ActorPlan.data` and the `ContinuousChannelPlan` events! By adding two read methods to `PlanBuilder`:
- `scene.actor_data::<T>(&actor)` — reads the current recipe payload from `scene.plan.actors`
- `scene.latest_literal(&actor, property)` — reads the latest authored target of `actor.property` from `scene.plan.continuous_channels`

every actor handle in the library (`Stage`, `Caption`, `Callout`, `RollingNumber`, `Tree`, `Plot`, `Lanes`, `Sequence`, `Video`) becomes a **stateless `Clone` value** whose methods take `&self` and return `impl Beat + '_`!

### Cross-Actor Composition Example

Because every actor's methods return `impl Beat` over `&mut PlanBuilder`, a single `scene.at(...)` expression can orchestrate a `Stage`, a `RollingNumber`, a `Callout`, a `Caption`, and a sound effect together:

```ts
let mut scene = PlanBuilder::new("cross-actor", 6 * SECOND);
let stage = Stage::declare(&mut scene, "stage", &stage_plan)?;
let version = RollingNumber::declare(&mut scene, "version", version_plan)?;
let note = Callout::declare(&mut scene, "note", &callout_plan)?;
let footer = Caption::declare(&mut scene, "footer", &footer_plan)?;

scene.at(
    0,
    stage
        .settle_in("client")
        .with(version.show())
        .with(note.show())
        .then(stage.connect("link", 0.6))
        .then(
            stage
                .send("hello", 0.8)
                .with(sound("send", "sfx/send.wav", millis(150), -10.0)),
        )
        .then(all![
            stage.land("server"),
            stage.jolt([1.0, 0.0], 0.6),
            version.roll("rc.117"),
            note.move_to("server").then(note.emphasize()),
            footer.type_in(50.0, 0.6),
        ]),
);
```

---

## 5. Spatial Composition (`psychopomp::layout`)

Borrowing from Haskell's `diagrams` library (envelopes/bounding regions, relative placement, and distribution) while respecting `PLAN.md` (*"Layout produces targets, not animation"*), `psychopomp::layout` provides pure, GPU-free 2.5D geometry values:

```ts
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub center: [f32; 3],
    pub size: [f32; 2],
}
```

- **From elements**: `Placement::card(at, size)`, `Placement::orb(at, radius)`, `Placement::of(&stage_plan, id)`
- **Relative anchors**: `p.below(gap)`, `p.above(gap)`, `p.beside_right(gap, size)`, `p.beside_left(gap, size)`, `p.stack_below(gap, size)`, `p.stack_above(gap, size)`, `p.align_left(inset, y_offset)`
- **Distributions**: `layout::row(center, pitch, count)`, `layout::column(center, pitch, count)`, `layout::spread_x(left, right, y, z, count)`
- **Spatial Laws**:
  1. *Opposite-Edge Round-trip*: `a.beside_right(gap, sz).beside_left(gap, a.size) == a` and `a.stack_below(gap, sz).stack_above(gap, a.size) == a`.
  2. *Translation Equivariance*: `p.translate(delta).below(gap) == p.below(gap) + delta`.
  3. *Centroid Invariance*: the arithmetic mean of `row(center, pitch, n)` and `column(center, pitch, n)` equals `center`.

---

## 6. Before & After Call Sites

### Call Site 1: `scenes/hello` (`scenes/hello/src/bin/hello_dsl.rs`)

**Before:**
```ts
let mut scene = PlanBuilder::new("hello", 4 * SECOND);
let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;
let ready = stage.settle_in(&mut scene, "client", 0); // the card drifts into place
let wired = stage.connect(&mut scene, "link", ready, 0.6); // the wire draws on
let landed = stage.send(&mut scene, "hello", wired + SECOND / 2, 0.8); // a packet flies
stage.land(&mut scene, "server", landed); // the orb lights up
stage.jolt(&mut scene, landed, [1.0, 0.0], 0.6); // and the camera takes the hit
```

**After:**
```ts
let mut scene = PlanBuilder::new("hello", 4 * SECOND);
let stage = Stage::declare(&mut scene, "stage", &plan)?;
scene.at(
    0,
    stage
        .settle_in("client")
        .then(stage.connect("link", 0.6))
        .then_after(SECOND / 2, stage.send("hello", 0.8))
        .then(stage.land("server").also(stage.jolt([1.0, 0.0], 0.6))),
);
```

### Call Site 2: The Rewind Act in `scenes/2password` (Stage + 3 Chips + Footer + SFX)

**Before (`scenes/2password/src/main.rs`):**
```ts
let switch = problem.end() + seconds(0.25);
s.clock_for(sc, "post.rewind", switch, 1.4);
s.hit(sc, "post.chroma", switch, 0.12, 0.0);
sound(sc, "rewind", LAUNCH, switch - seconds(0.1), -14.0);
raw_chip.hide(sc, switch);
footer_problem.hide(sc, switch);
let mut rewind_chip = chip(sc, "chip-rewind", Tone::Accent, "◀◀ rewind")?;
rewind_chip.show(sc, switch + seconds(0.25));
rewind_chip.hide(sc, switch + seconds(1.5));
let mut fixed_chip = chip(sc, "chip-fixed", Tone::Success, "with 2password")?;
fixed_chip.show(sc, switch + seconds(1.65));
s.to(sc, "leak-line.opacity", switch, 0.0, 0.4);
s.to(sc, "agent.alarm", switch, 0.0, 0.4);
s.to(sc, "agent.status", switch + seconds(0.2), 0.0, 0.9);
s.to(sc, "agent.glow", switch, 0.0, 0.6);
s.to(sc, "op.status", switch + seconds(0.2), 0.0, 0.6);
s.to(sc, "direct.emphasis", switch, 0.0, 0.6);
s.to(sc, "camera.x", switch, 0.0, 1.6);
s.to(sc, "camera.z", switch, 0.0, 1.8);
s.to(sc, "camera.focus", switch, 0.0, 1.0);
```

**After (`scenes/2password/src/bin/2password_dsl.rs`):**
```ts
sc.at(
    problem.end() + seconds(0.25),
    all![
        s.clock_for("post.rewind", 1.4),
        s.hit("post.chroma", 0.12, 0.0),
        sfx("rewind", LAUNCH, -14.0).early(seconds(0.1)),
        raw_chip.hide(),
        footer_problem.hide(),
        rewind_chip.show().after(seconds(0.25)),
        rewind_chip.hide().after(seconds(1.5)),
        fixed_chip.show().after(seconds(1.65)),
        s.to("leak-line.opacity", 0.0, 0.4),
        s.to("agent.alarm", 0.0, 0.4),
        s.to("agent.status", 0.0, 0.9).after(seconds(0.2)),
        s.to("agent.glow", 0.0, 0.6),
        s.to("op.status", 0.0, 0.6).after(seconds(0.2)),
        s.to("direct.emphasis", 0.0, 0.6),
        s.to("camera.x", 0.0, 1.6),
        s.to("camera.z", 0.0, 1.8),
        s.to("camera.focus", 0.0, 1.0),
    ],
);
```
Every actor (`s`, `raw_chip`, `footer_problem`, `rewind_chip`, `fixed_chip`) and `sfx` sits inside one `sc.at(...)` block anchored at `problem.end() + seconds(0.25)`, with relative offsets (`.early(0.1s)`, `.after(0.25s)`, `.after(1.5s)`) expressed locally instead of repeating `switch + ...` 19 times.

---

## 7. Explicit Decision Points

### Decision Point 1: Actor-Method Beat Constructors (`s.send(...)`) vs Free Functions (`stage::send(...)`)

- **Option A (Actor-method constructors on immutable value handles: `stage.send("hello", 0.8)`, `footer.type_in(46.0, 0.8)`, `version.roll("rc.117")`, `note.show()`):**
  Every actor handle (`Stage`, `Caption`, `Callout`, `RollingNumber`, `Tree`, `Plot`, `Lanes`, `Sequence`, `Video`) is an immutable `Clone` value whose methods take `&self` and return `impl Beat + '_` playing against `&mut PlanBuilder`.
- **Option B (Free functions in per-actor modules: `stage::send("hello", 0.8)` with a per-actor `StageCtx`):**
  Bind a single actor in a specialized `Score<StageCtx>` so free functions don't name the actor receiver.
- **Recommendation: Option A (implemented).**
  Option B breaks down as soon as a scene has more than one actor (e.g. a `Stage` plus three `Caption` chips, two `Callout`s, and a `RollingNumber`), forcing non-Stage actors out of the score or requiring escape hatches. With Option A, `s.send("hello", 0.8)` is *shorter* than `stage::send("hello", 0.8)` (`s.` vs `stage::`), names which caption/callout/number is acting (`raw_chip.hide()` vs `rewind_chip.show()`), and lets all actors compose in one `all![...]` over `&mut PlanBuilder`.

### Decision Point 2: Operator Sugar (`>>`, `&`) vs Fluent Methods (`.then()`, `.also()`) + `chain!` / `all!`

- **Option A (Fluent methods `.then()`, `.then_after()`, `.also()`, `.with()`, `.on_end()`, `.after()`, `.early()` + `chain![...]` / `all![...]`):**
  Methods on `Beat` plus variadic macros for flat sequences and parallel groups.
- **Option B (`std::ops::Shr` `>>` for sequence and `BitAnd` `&` for parallel):**
  Operator overloading on a concrete `BeatBox` wrapper.
- **Recommendation: Option A (implemented).**
  Operator overloading on generic traits in Rust requires newtype wrapping at every leaf, introduces precedence traps between `>>` and `&`, and formats poorly under `rustfmt`.

### Decision Point 3: Same-Timestamp Parallel Writes on One Channel

- **Option A (Stable sort by `at_nanos` in `PlanBuilder::finish` + `score::find_write_conflicts(&plan)` lint):**
  Preserve source order for equal timestamps (`Set` followed by `Spring`/`Ease`) while sorting distinct timestamps chronologically, and provide `find_write_conflicts` to flag accidental same-time competing writes.
- **Option B (Hard error during `Beat::play` on any same-time same-channel write):**
  Reject any two writes at the same nanosecond on the same channel.
- **Recommendation: Option A (implemented).**
  Existing compound beats (`settle_in`, `hit`, `clock_for`) rely on `Set(at)` followed immediately by `Spring(at)` or `Ease(at)` at the same timestamp. Option A keeps 100% byte-identical `ScenePlan` output while catching genuine parallel collisions.

### Decision Point 4: Transitioning `*Actor` vs `score::*` Value Handles at Merge Time

- **Option A (Keep both `StageActor` imperative methods and `score::Stage` value handles, with `From<StageActor>` and `.beats()`):**
  Zero existing scene files in `scenes/*` need edits when merging with the 10 parallel branches in flight; scenes opt into `score::{Stage, Caption, ...}` incrementally.
- **Option B (Replace `StageActor` methods in-place with Beat-returning methods across all `scenes/*` at once):**
  Single handle type per recipe, migrating all 18 scene crates in one commit.
- **Recommendation: Option A during multi-branch integration, then Option B once sibling branches merge.**
