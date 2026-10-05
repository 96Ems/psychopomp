# Psychopomp Authoring Language Design (`DSL.md`)

> *"First define what the things **mean** (their denotation); then choose the combinators whose meanings compose; then check the algebraic laws they obey."*
> — Conal Elliott & Simon Peyton Jones

Psychopomp's renderer has a crisp semantic core: a **Scene Plan** is a declarative collection of actors and tracks, and rendering is a pure function `Time → Pixels` with temporal shutter accumulation.

`psychopomp::score` and `psychopomp::layout` provide the composable authoring algebra over that core, governed by one structural rule:

> **The `PlanBuilder` is the only mutable thing; actor handles are values.**

---

## 1. Why the Old Relative `Animation` Algebra Failed

Commit `20fc32c` removed an earlier `Animation` / `Composition` tree (`Sequence`, `Parallel`, `Delay`, `Hold`). It failed for five reasons:

1. **It bypassed Scene Plans.** `Composition` compiled directly into in-memory renderer structs rather than emitting a `ScenePlan` JSON artifact.
2. **It conflated *physical settling time* with *choreographic span*.** In the old `Animation::Spring`, a spring's sequential duration was `profile.advance_time()` (~1.1 s until position and velocity fall below `0.001`). In real motion design, the next beat almost never waits for asymptotic rest: `settle_in` is ready to wire after `500 ms` while its 0.6 s spring is still settling; `send` completes on *arrival* while its trail cools for `4.0 s`; a camera move (`to("camera.x", ...)`) is an impulse (`0 ns` choreographic wait) that glides in the background while the action continues.
3. **It accumulated `f32` seconds instead of exact `u64` nanoseconds.** Repeated `f32` additions (`0.1_f32 + 0.2_f32`) drifted from `u64` media timestamps, breaking associativity.
4. **It was a closed ADT coupled to one recipe.** `Composition` had hardcoded enum variants (`Animate`, `Annotate`, `Task`, `Play`) and could not be extended by new recipes.
5. **It was purely relative from `t = 0`.** Narrated explainers (`2password`, `pr-walkthrough`) are **hybrid**: beats are anchored to spoken phrases (`p("asks")`), chained causally (`settle_in` → `connect` → `send` → `land`), joined by causal guards (`f("injected").not_before(contact.after(millis(340)))`), or back-timed to land *on* a word (`send_arriving`).

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

When event $e_i$ is a `Spring(target, profile)` at time $t_i$, its segment is initialized with the **sampled position and velocity** of the preceding trajectory at $t_i$:

$$\mathsf{initial}_i = \mathsf{state}(t_i^-)$$

#### Denotation of a `Span` (Choreographic Interval)

$$\mathsf{Span} = \{ (\mathit{start}, \mathit{end}) \in \mathbb{T} \times \mathbb{T} \mid \mathit{start} \le \mathit{end} \}$$

- `span.start`: when the beat was cued.
- `span.end`: when the beat's primary action completes (when a panel is ready, a wire makes contact, a packet lands, a command finishes typing, or a camera shot lands).
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

Actor handles (`Stage`, `Camera`, `Caption`, `Callout`, `RollingNumber`, `Tree`, `Plot`, `Lanes`, `Sequence`, `Video`, `Terminal`, `Chat`, `ChangedFiles`, `LowerThird`, `Checklist`, `Meter`, `Bars`, `Subtitles`, `Confetti`, `Text`, `Image`, `Lens`, `Diagnostic`, `Hover`, `Cursor`, `InlayHint`) and sound resources (`sfx::Sfx::beat`, `psychopomp_media::Audio::beat`) are `Clone` values whose methods take `&self` and return `impl Beat + '_`.

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
| **Cue Sheet** | `at!(sc, t => a, b, c)` | `sc.at(t, all![a, b, c])` | Cue multiple parallel beats at $t$ |
| **Accompany** | `a.with(b)` | Let $(s_1, w_1) = \llbracket a \rrbracket(t)$, $(s_2, w_2) = \llbracket b \rrbracket(t)$ in $(s_1, w_1 \mathbin{+\!+} w_2)$ | Play subordinate action $b$ at $a$'s start; keep $a$'s span |
| **On End** | `a.on_end(b)` | Let $(s_1, w_1) = \llbracket a \rrbracket(t)$, $(s_2, w_2) = \llbracket b \rrbracket(s_1.\mathit{end})$ in $(s_1, w_1 \mathbin{+\!+} w_2)$ | Trigger $b$ at $a$'s end (e.g. sound/flash on contact); keep $a$'s span |
| **Delay** | `a.after(d)` | Let $(s, w) = \llbracket a \rrbracket(t + d)$ in $((t, s.\mathit{end}), w)$ | Wait $d$ ns after $t$, then play $a$ |
| **Lead / Pre-roll** | `a.early(d)` | Let $t' = t \ominus d$, $(s, w) = \llbracket a \rrbracket(t')$ in $((t', s.\mathit{end}), w)$ | Start $d$ ns *before* $t$ (clamped at 0) |
| **Stagger** | `stagger(gap, items, f)` | $\mathsf{all}_{i=0}^{n-1} f(x_i).\mathsf{after}(i \cdot \mathit{gap})$ | Ripple $f(x_i)$ spaced by $\mathit{gap}$ |
| **Each** | `each(items, f)` | $\mathsf{stagger}(0, \mathit{items}, f)$ | Fan out $f(x_i)$ simultaneously at $t$ |

### 3.2 `CueTime` Anchors (`u64` and `Span`)

Both `u64` timestamps (`spoken.at("phrase")`) and `Span`s returned by `scene.at(...)` / `at!(...)` implement `CueTime`:
- `t.after(delta)`: `t.cue_nanos() + delta`
- `t.early(lead)`: `t.cue_nanos().saturating_sub(lead)`
- `t.not_before(earliest)`: `t.cue_nanos().max(earliest.cue_nanos())`
- `t.reply()`: `reply_after(t.cue_nanos())` (`340 ms` packet gather + `80 ms` reaction)

### 3.3 Algebraic Laws

Because timestamps are exact `u64` nanoseconds, these laws hold **to the nanosecond** (verified by unit tests and a 200-random-AST rewrite property test in `score::tests`):

1. **Sequence Monoid**: `a.then(b).then(c) == a.then(b.then(c))` and `empty().then(a) == a == a.then(empty())`
2. **Hold Fusion & Delay Isomorphism**: `hold(d1).then(hold(d2)) == hold(d1 + d2)` and `hold(d).then(a) == a.after(d)` and `a.after(d1).after(d2) == a.after(d1 + d2)`
3. **Sequence-Delay Left Shift**: `a.then(b).after(d) == a.after(d).then(b)`
4. **Parallel Monoid & Delay Distributivity**: `a.also(b).also(c) == a.also(b.also(c))` and `a.also(b).after(d) == a.after(d).also(b.after(d))`
5. **Impulse Absorption**: for any impulse $p$ ($\mathit{duration}(p) = 0$), `p.then(a) == p.also(a)` and `a.on_end(p) == a.then(p)`
6. **Stagger & Each Decomposition**: `stagger(gap, [x0, x1, x2], f) == f(x0).also(f(x1).after(gap)).also(f(x2).after(2 * gap))`
7. **Spring Retargeting Commutativity**: `PlanBuilder::finish()` stably sorts each channel's events by `at_nanos` (`sort_by_key`), so parallel writes at distinct timestamps commute (`a.also(b) == b.also(a)`) on both `ScenePlan` JSON and sampled `Timeline` `(position, velocity)` trajectories, while equal-timestamp writes preserve source order (`Set` followed by `Spring`/`Ease`).

---

## 4. Before & After Call Sites

### Call Site 1: Cross-Actor Beat (`Stage` + `Camera` + `Terminal` + `Subtitles` + `sfx`)

```ts
let mut scene = PlanBuilder::new("camera-terminal-subtitles", 8 * SECOND);
let stage = Stage::declare(&mut scene, "stage", &stage_plan)?;
let cam = stage.camera();
let term = Terminal::declare(&mut scene, "term", TerminalPlan::new([120.0, 680.0], 720.0, 6).titled("zsh"))?;
let subs = Subtitles::declare(&mut scene, "subs", &SubtitlesPlan::from_spoken(&spoken, [960.0, 980.0], 900.0))?;

at!(scene, 0 =>
    cam.establish(160.0, 1.4),
    stage
        .settle_in("client")
        .with(term.show())
        .then(stage.connect("link", 0.6))
        .on_end(sfx::TICK.beat("wired", -18.0))
        .then(
            stage
                .send("hello", 0.8)
                .with(cam.follow("hello", Move::Spring(0.5)))
                .with(term.type_command("curl -s http://server/hello")),
        )
        .then(all![
            stage.land("server"),
            cam.release(Move::Spring(0.8)),
            term.print_text("200 OK", Tone::Success),
            sfx::CONFIRM.beat("ok", -12.0),
        ])
        .then_after(SECOND, subs.hide()),
);
```

### Call Site 2: `scenes/2password` Request/Reply & Rewind Acts

**Before (`scenes/2password/src/main.rs`):**
```ts
let everything = l("everything");
s.to(sc, "agent.status", everything, 4.0, 0.3);
let find = s.send(sc, "find", everything - seconds(0.2), 0.6);
sfx::SEND.play(sc, "find", everything - seconds(0.2), -11.0);
s.land(sc, "layer", find);
s.to(sc, "layer.status", find, 1.0, 0.3);
let lookup = s.send(
    sc,
    "lookup",
    l("just once")
        .saturating_sub(seconds(0.35))
        .not_before(reply_after(find)),
    0.55,
);
sfx::SEND.play(sc, "lookup", lookup - seconds(0.55), -11.0);
s.land(sc, "op", lookup);
let fetch = s.send(sc, "fetch", reply_after(lookup), 0.45);
s.land(sc, "vault", fetch);
```

**After (`scenes/2password/src/bin/2password_dsl.rs`):**
```ts
let find = at!(sc, l("everything") =>
    s.to("agent.status", 4.0, 0.3),
    s.send("find", 0.6)
        .with(sfx::SEND.beat("find", -11.0))
        .early(seconds(0.2))
        .then(s.land("layer").also(s.to("layer.status", 1.0, 0.3))),
);
let lookup = sc.at(
    l("just once").early(seconds(0.35)).not_before(find.reply()),
    s.send("lookup", 0.55)
        .with(sfx::SEND.beat("lookup", -11.0))
        .then(s.land("op")),
);
let fetch = sc.at(lookup.reply(), s.send("fetch", 0.45).then(s.land("vault")));
```

---

## 5. Explicit Decision Points

1. **Actor-Method Beat Constructors (`s.send(...)`) vs Free Functions (`stage::send(...)`)**
   - *Recommendation*: **Actor-method constructors on immutable value handles** (implemented). Every actor handle (`Stage`, `Camera`, `Caption`, `Callout`, `RollingNumber`, `Tree`, `Plot`, `Lanes`, `Sequence`, `Video`, `Terminal`, `Chat`, `ChangedFiles`, `LowerThird`, `Checklist`, `Meter`, `Bars`, `Subtitles`, `Confetti`, `Text`, `Image`, `Lens`, `Diagnostic`, `Hover`, `Cursor`, `InlayHint`) is a `Clone` value whose methods take `&self` and return `impl Beat + '_` over `&mut PlanBuilder`.
2. **Fluent Combinators (`.then()`, `.also()`, `at!`) vs Bitwise Operators (`>>`, `&`)**
   - *Recommendation*: **Fluent methods + `at!` / `all!` / `chain!`** (implemented). Avoids newtype wrappers, operator precedence traps, and `rustfmt` line-breaking artifacts.
3. **Same-Timestamp Parallel Writes on One Channel**
   - *Recommendation*: **Stable sort by `at_nanos` in `PlanBuilder::finish` + `score::find_write_conflicts(&plan)`** (implemented). Preserves the `Set(at)`-then-`Spring(at)` idiom while flagging competing parallel writes.
