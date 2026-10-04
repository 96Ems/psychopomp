//! Composable choreography (`Score`, `Beat`, `Span`, and `CueTime`).
//!
//! ## Denotation
//!
//! A [`Beat`] is a pure schedule transformer:
//!
//! ```text
//! ⟦Beat⟧ : Time (start_nanos: u64) -> (Span, Writes)
//! ```
//!
//! Given a start time in nanoseconds on the plan clock, playing a [`Beat`]
//! emits channel/media/cue writes into its context (such as a [`PlanBuilder`]
//! or [`StageScore`]) and returns the choreographic [`Span`] `[start, end]`
//! occupied by its primary action.
//!
//! Crucially, a beat's choreographic [`Span`] is distinct from the physical
//! settling tail of its springs:
//! - Impulses (`to`, `spring`, `ease`, `set`, `hit`, `land`, `jolt`, `sound`)
//!   have zero choreographic duration (`end == start`) so subsequent beats can
//!   fire immediately or after an explicit [`hold`] / [`Beat::after`].
//! - Phased actions (`settle_in`, `connect`, `send`, `type_in`) advance `end`
//!   to their semantic completion moment (when a panel is ready to wire, when a
//!   beam makes contact, when a packet lands, or when typing finishes).
//!
//! Every combinator compiles directly to ordinary [`PlanBuilder`] events—no
//! runtime scene graph, callbacks, or Scene Plan schema changes.

use std::path::PathBuf;

use crate::{
    author::{PlanBuilder, whole_millis},
    math::easing::Ease,
    plan::{MediaKindPlan, MediaPlan, MediaRolePlan, ScenePlan, SpringPlan, TrackEventPlan},
    stage::{StageActor, packet},
};

/// One millisecond on the plan clock, in nanoseconds.
pub const MILLISECOND: u64 = 1_000_000;

/// `millis` whole milliseconds on the plan clock, in nanoseconds.
pub const fn millis(millis: u64) -> u64 {
    millis * MILLISECOND
}

/// Calibrated spring motion profiles from `explainer-motion/TECHNIQUES.md`.
pub struct Feel;

impl Feel {
    /// A rigid panel settling into place: 0.6 s, bounce 0.12.
    pub const PANEL: SpringPlan = Self::visual(0.6, 0.12);
    /// Ink following its panel, or a label fading: 0.36 s, no bounce.
    pub const CONTENT: SpringPlan = Self::visual(0.36, 0.0);
    /// A quick state change, such as a status cross-fade: 0.3 s, no bounce.
    pub const SNAP: SpringPlan = Self::visual(0.3, 0.0);
    /// A camera move with weight and a natural tail: 1.6 s, critically damped.
    pub const CAMERA: SpringPlan = Self::visual(1.6, 0.0);
    /// A hero landing with a little overshoot: 0.85 s, bounce 0.2.
    pub const LIVELY: SpringPlan = Self::visual(0.85, 0.2);

    /// Const-evaluatable equivalent of [`SpringPlan::visual`].
    pub const fn visual(duration: f32, bounce: f32) -> SpringPlan {
        SpringPlan {
            response_seconds: duration * 1.2,
            damping_ratio: 1.0 - bounce,
            position_threshold: 0.001,
            velocity_threshold: 0.001,
        }
    }
}

/// The choreographic interval `[start, end]` occupied by a [`Beat`] on the
/// plan clock, in nanoseconds (`start <= end`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Span {
    pub start: u64,
    pub end: u64,
}

impl Span {
    /// Construct a span `[start, end]`, clamping `end` to be at least `start`.
    pub const fn new(start: u64, end: u64) -> Self {
        let end = if end < start { start } else { end };
        Self { start, end }
    }

    /// Zero-duration span `[at, at]` for an impulse beat.
    pub const fn impulse(at: u64) -> Self {
        Self { start: at, end: at }
    }

    /// Choreographic duration `end - start` in nanoseconds.
    pub const fn duration(self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// `delta` nanoseconds after this span's `start`.
    pub const fn after_start(self, delta: u64) -> u64 {
        self.start + delta
    }

    /// `delta` nanoseconds before this span's `end` (clamped at zero).
    pub const fn before_end(self, delta: u64) -> u64 {
        self.end.saturating_sub(delta)
    }
}

/// Any plan-clock anchor that can cue a [`Beat`]: a raw `u64` nanosecond
/// timestamp (such as `Spoken::at("phrase")`) or the completion of a prior
/// [`Span`].
pub trait CueTime: Copy {
    /// Resolved nanosecond timestamp on the plan clock.
    fn cue_nanos(self) -> u64;

    /// `delta` nanoseconds after this cue time.
    fn after(self, delta: u64) -> u64 {
        self.cue_nanos() + delta
    }

    /// `lead` nanoseconds before this cue time, saturating at zero.
    fn early(self, lead: u64) -> u64 {
        self.cue_nanos().saturating_sub(lead)
    }

    /// This cue time, or `earliest` if `earliest` is later (`max`).
    ///
    /// Expresses causal guards such as `f("injected").not_before(contact.after(millis(340)))`.
    fn not_before(self, earliest: impl CueTime) -> u64 {
        self.cue_nanos().max(earliest.cue_nanos())
    }
}

impl CueTime for u64 {
    fn cue_nanos(self) -> u64 {
        self
    }
}

impl CueTime for Span {
    fn cue_nanos(self) -> u64 {
        self.end
    }
}

/// A composable unit of choreography over a mutable authoring context `Ctx`.
pub trait Beat<Ctx>: Sized {
    /// Emit this beat's writes into `ctx` starting at `at` nanoseconds, and
    /// return its occupied choreographic [`Span`].
    fn play(self, ctx: &mut Ctx, at: u64) -> Span;

    /// Sequential composition: play `self` at `at`, then play `next` at
    /// `self.end`.
    ///
    /// Obeys associativity (`a.then(b).then(c) == a.then(b.then(c))`) and
    /// identity (`empty().then(a) == a == a.then(empty())`).
    fn then<B>(self, next: B) -> Then<Self, B> {
        Then {
            first: self,
            second: next,
        }
    }

    /// Play `self`, wait `gap` nanoseconds after `self.end`, then play `next`.
    ///
    /// Equivalent to `self.then(next.after(gap))` and `self.then(hold(gap)).then(next)`.
    fn then_after<B>(self, gap: u64, next: B) -> Then<Self, After<B>> {
        self.then(After {
            delay: gap,
            beat: next,
        })
    }

    /// Parallel composition: start both `self` and `other` at `at`; complete
    /// when both have completed (`max(self.end, other.end)`).
    ///
    /// Obeys associativity, identity (`empty()`), and delay distributivity
    /// (`a.also(b).after(d) == a.after(d).also(b.after(d))`).
    fn also<B>(self, other: B) -> Also<Self, B> {
        Also {
            left: self,
            right: other,
        }
    }

    /// Accompany `self` with a subordinate beat `subordinate` that starts at
    /// `self.start`, while keeping `self`'s choreographic [`Span`].
    fn with<B>(self, subordinate: B) -> With<Self, B> {
        With {
            primary: self,
            subordinate,
        }
    }

    /// Trigger a subordinate `reaction` when `self` finishes (`self.end`),
    /// while preserving `self`'s [`Span`].
    fn on_end<B>(self, reaction: B) -> OnEnd<Self, B> {
        OnEnd {
            primary: self,
            reaction,
        }
    }

    /// Delay this beat by `delay` nanoseconds relative to the cue time.
    ///
    /// The returned span is `[at, inner.end]` where `inner` starts at `at + delay`.
    fn after(self, delay: u64) -> After<Self> {
        After { delay, beat: self }
    }

    /// Start this beat `lead` nanoseconds *before* the cue time (saturating at 0).
    fn early(self, lead: u64) -> Early<Self> {
        Early { lead, beat: self }
    }

    /// Extend this beat's sequential span by `hold_nanos` of stillness after it finishes.
    fn hold_for(self, hold_nanos: u64) -> Then<Self, Hold> {
        self.then(hold(hold_nanos))
    }
}

impl<Ctx, F> Beat<Ctx> for F
where
    F: FnOnce(&mut Ctx, u64) -> Span,
{
    fn play(self, ctx: &mut Ctx, at: u64) -> Span {
        self(ctx, at)
    }
}

/// The identity beat: writes nothing and occupies `[at, at]` (`duration == 0`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Empty;

/// Identity beat that emits no writes and advances time by `0 ns`.
pub const fn empty() -> Empty {
    Empty
}

impl<Ctx> Beat<Ctx> for Empty {
    fn play(self, _ctx: &mut Ctx, at: u64) -> Span {
        Span::impulse(at)
    }
}

impl Empty {
    pub const fn then<B>(self, next: B) -> Then<Self, B> {
        Then {
            first: self,
            second: next,
        }
    }

    pub const fn also<B>(self, other: B) -> Also<Self, B> {
        Also {
            left: self,
            right: other,
        }
    }
}

/// A pure wait of `duration_nanos` that emits no writes and occupies
/// `[at, at + duration_nanos]`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hold {
    pub duration_nanos: u64,
}

/// Wait `duration_nanos` on the timeline without writing any events.
pub const fn hold(duration_nanos: u64) -> Hold {
    Hold { duration_nanos }
}

impl<Ctx> Beat<Ctx> for Hold {
    fn play(self, _ctx: &mut Ctx, at: u64) -> Span {
        Span::new(at, at + self.duration_nanos)
    }
}

impl Hold {
    pub const fn then<B>(self, next: B) -> Then<Self, B> {
        Then {
            first: self,
            second: next,
        }
    }

    pub const fn also<B>(self, other: B) -> Also<Self, B> {
        Also {
            left: self,
            right: other,
        }
    }
}

/// A zero-duration beat (`Span::impulse(at)`) constructed from a closure.
pub struct Impulse<F> {
    f: F,
}

/// Construct an impulse beat that writes at `at` and advances the cursor by `0 ns`.
pub fn impulse<Ctx, F: FnOnce(&mut Ctx, u64)>(f: F) -> Impulse<F> {
    Impulse { f }
}

impl<Ctx, F: FnOnce(&mut Ctx, u64)> Beat<Ctx> for Impulse<F> {
    fn play(self, ctx: &mut Ctx, at: u64) -> Span {
        (self.f)(ctx, at);
        Span::impulse(at)
    }
}

/// A span-bearing beat constructed from a closure returning its completion time `end`.
pub struct Action<F> {
    f: F,
}

/// Construct a beat from an action that starts at `at` and returns its `end` timestamp.
pub fn action<Ctx, F: FnOnce(&mut Ctx, u64) -> u64>(f: F) -> Action<F> {
    Action { f }
}

impl<Ctx, F: FnOnce(&mut Ctx, u64) -> u64> Beat<Ctx> for Action<F> {
    fn play(self, ctx: &mut Ctx, at: u64) -> Span {
        let end = (self.f)(ctx, at);
        Span::new(at, end)
    }
}

/// Sequential composition of two beats (`first.then(second)`).
#[derive(Clone, Copy, Debug)]
pub struct Then<A, B> {
    pub first: A,
    pub second: B,
}

impl<Ctx, A: Beat<Ctx>, B: Beat<Ctx>> Beat<Ctx> for Then<A, B> {
    fn play(self, ctx: &mut Ctx, at: u64) -> Span {
        let first_span = self.first.play(ctx, at);
        let second_span = self.second.play(ctx, first_span.end);
        Span::new(at, second_span.end)
    }
}

impl<A, B> Then<A, B> {
    pub const fn then<C>(self, next: C) -> Then<Self, C> {
        Then {
            first: self,
            second: next,
        }
    }

    pub const fn then_after<C>(self, gap: u64, next: C) -> Then<Self, After<C>> {
        Then {
            first: self,
            second: After {
                delay: gap,
                beat: next,
            },
        }
    }

    pub const fn also<C>(self, other: C) -> Also<Self, C> {
        Also {
            left: self,
            right: other,
        }
    }

    pub const fn with<C>(self, subordinate: C) -> With<Self, C> {
        With {
            primary: self,
            subordinate,
        }
    }

    pub const fn on_end<C>(self, reaction: C) -> OnEnd<Self, C> {
        OnEnd {
            primary: self,
            reaction,
        }
    }

    pub const fn after(self, delay: u64) -> After<Self> {
        After { delay, beat: self }
    }

    pub const fn early(self, lead: u64) -> Early<Self> {
        Early { lead, beat: self }
    }
}

/// Parallel composition of two beats (`left.also(right)`).
#[derive(Clone, Copy, Debug)]
pub struct Also<A, B> {
    pub left: A,
    pub right: B,
}

impl<Ctx, A: Beat<Ctx>, B: Beat<Ctx>> Beat<Ctx> for Also<A, B> {
    fn play(self, ctx: &mut Ctx, at: u64) -> Span {
        let left_span = self.left.play(ctx, at);
        let right_span = self.right.play(ctx, at);
        Span::new(at, left_span.end.max(right_span.end))
    }
}

/// Primary beat accompanied by a subordinate beat at `start` (`primary.with(subordinate)`).
#[derive(Clone, Copy, Debug)]
pub struct With<A, B> {
    pub primary: A,
    pub subordinate: B,
}

impl<Ctx, A: Beat<Ctx>, B: Beat<Ctx>> Beat<Ctx> for With<A, B> {
    fn play(self, ctx: &mut Ctx, at: u64) -> Span {
        let span = self.primary.play(ctx, at);
        let _ = self.subordinate.play(ctx, at);
        span
    }
}

/// Primary beat that triggers `reaction` at `primary.end`, keeping `primary`'s span.
#[derive(Clone, Copy, Debug)]
pub struct OnEnd<A, B> {
    pub primary: A,
    pub reaction: B,
}

impl<Ctx, A: Beat<Ctx>, B: Beat<Ctx>> Beat<Ctx> for OnEnd<A, B> {
    fn play(self, ctx: &mut Ctx, at: u64) -> Span {
        let span = self.primary.play(ctx, at);
        let _ = self.reaction.play(ctx, span.end);
        span
    }
}

/// A beat delayed by `delay` nanoseconds (`beat.after(delay)`).
#[derive(Clone, Copy, Debug)]
pub struct After<B> {
    pub delay: u64,
    pub beat: B,
}

impl<Ctx, B: Beat<Ctx>> Beat<Ctx> for After<B> {
    fn play(self, ctx: &mut Ctx, at: u64) -> Span {
        let inner = self.beat.play(ctx, at + self.delay);
        Span::new(at, inner.end)
    }
}

/// A beat shifted earlier by `lead` nanoseconds (`beat.early(lead)`).
#[derive(Clone, Copy, Debug)]
pub struct Early<B> {
    pub lead: u64,
    pub beat: B,
}

impl<Ctx, B: Beat<Ctx>> Beat<Ctx> for Early<B> {
    fn play(self, ctx: &mut Ctx, at: u64) -> Span {
        let shifted = at.saturating_sub(self.lead);
        self.beat.play(ctx, shifted)
    }
}

/// Sequential composition of a homogeneous collection of beats.
pub struct Seq<I> {
    beats: I,
}

/// Play an iterator of beats one after another.
pub fn seq<I>(beats: I) -> Seq<I> {
    Seq { beats }
}

impl<Ctx, B: Beat<Ctx>, I: IntoIterator<Item = B>> Beat<Ctx> for Seq<I> {
    fn play(self, ctx: &mut Ctx, at: u64) -> Span {
        let mut cursor = at;
        for beat in self.beats {
            cursor = beat.play(ctx, cursor).end;
        }
        Span::new(at, cursor)
    }
}

/// Parallel composition of a homogeneous collection of beats.
pub struct All<I> {
    beats: I,
}

/// Start an iterator of beats at the same timestamp; finish when the last finishes.
pub fn all<I>(beats: I) -> All<I> {
    All { beats }
}

impl<Ctx, B: Beat<Ctx>, I: IntoIterator<Item = B>> Beat<Ctx> for All<I> {
    fn play(self, ctx: &mut Ctx, at: u64) -> Span {
        let mut end = at;
        for beat in self.beats {
            end = end.max(beat.play(ctx, at).end);
        }
        Span::new(at, end)
    }
}

/// Staggered parallel composition: item `i` starts at `at + i * gap`.
pub struct Stagger<I, F> {
    gap: u64,
    items: I,
    make_beat: F,
}

/// Start one beat per item, spaced `gap` nanoseconds apart from `at`.
///
/// Satisfies the law:
/// `stagger(gap, [x0, x1, ...], f) == all(items.enumerate().map(|(i, x)| f(x).after(i * gap)))`.
pub fn stagger<T, I, B, F>(gap: u64, items: I, make_beat: F) -> Stagger<I, F>
where
    I: IntoIterator<Item = T>,
    F: FnMut(T) -> B,
{
    Stagger {
        gap,
        items,
        make_beat,
    }
}

impl<Ctx, T, I, B, F> Beat<Ctx> for Stagger<I, F>
where
    I: IntoIterator<Item = T>,
    B: Beat<Ctx>,
    F: FnMut(T) -> B,
{
    fn play(mut self, ctx: &mut Ctx, at: u64) -> Span {
        let mut latest_end = at;
        for (index, item) in self.items.into_iter().enumerate() {
            let item_start = at + self.gap * index as u64;
            let span = (self.make_beat)(item).play(ctx, item_start);
            latest_end = latest_end.max(span.end);
        }
        Span::new(at, latest_end)
    }
}

///Indexed variant of [`stagger`] when the beat depends on its `0..` index.
pub struct StaggerIndexed<I, F> {
    gap: u64,
    items: I,
    make_beat: F,
}

/// Start one beat per `(index, item)`, spaced `gap` nanoseconds apart from `at`.
pub fn stagger_indexed<T, I, B, F>(gap: u64, items: I, make_beat: F) -> StaggerIndexed<I, F>
where
    I: IntoIterator<Item = T>,
    F: FnMut(usize, T) -> B,
{
    StaggerIndexed {
        gap,
        items,
        make_beat,
    }
}

impl<Ctx, T, I, B, F> Beat<Ctx> for StaggerIndexed<I, F>
where
    I: IntoIterator<Item = T>,
    B: Beat<Ctx>,
    F: FnMut(usize, T) -> B,
{
    fn play(mut self, ctx: &mut Ctx, at: u64) -> Span {
        let mut latest_end = at;
        for (index, item) in self.items.into_iter().enumerate() {
            let item_start = at + self.gap * index as u64;
            let span = (self.make_beat)(index, item).play(ctx, item_start);
            latest_end = latest_end.max(span.end);
        }
        Span::new(at, latest_end)
    }
}

/// Compose a heterogeneous list of beats in sequence using [`Beat::then`].
#[macro_export]
macro_rules! chain {
    () => {
        $crate::score::empty()
    };
    ($first:expr $(,)?) => {
        $first
    };
    ($first:expr, $($rest:expr),+ $(,)?) => {
        $crate::score::Beat::then($first, $crate::chain!($($rest),+))
    };
}

/// Compose a heterogeneous list of beats in parallel using [`Beat::also`].
#[macro_export]
macro_rules! all {
    () => {
        $crate::score::empty()
    };
    ($first:expr $(,)?) => {
        $first
    };
    ($first:expr, $($rest:expr),+ $(,)?) => {
        $crate::score::Beat::also($first, $crate::all!($($rest),+))
    };
}

pub use {all as parallel, chain};

/// A score runner bound to a mutable authoring context `Ctx`.
pub struct Score<'a, Ctx> {
    ctx: &'a mut Ctx,
}

impl<'a, Ctx> Score<'a, Ctx> {
    pub fn new(ctx: &'a mut Ctx) -> Self {
        Self { ctx }
    }

    pub fn context(&mut self) -> &mut Ctx {
        self.ctx
    }

    /// Play `beat` anchored at `time` on the plan clock and return its [`Span`].
    pub fn at(&mut self, time: impl CueTime, beat: impl Beat<Ctx>) -> Span {
        beat.play(self.ctx, time.cue_nanos())
    }
}

/// Combined authoring context for a [`StageActor`] and its [`PlanBuilder`].
pub struct StageCtx<'a> {
    pub stage: &'a mut StageActor,
    pub scene: &'a mut PlanBuilder,
}

/// A [`Score`] specialized for [`StageActor`] + [`PlanBuilder`] choreography.
pub struct StageScore<'a> {
    ctx: StageCtx<'a>,
}

impl<'a> StageScore<'a> {
    pub fn new(stage: &'a mut StageActor, scene: &'a mut PlanBuilder) -> Self {
        Self {
            ctx: StageCtx { stage, scene },
        }
    }

    pub fn stage(&mut self) -> &mut StageActor {
        self.ctx.stage
    }

    pub fn scene(&mut self) -> &mut PlanBuilder {
        self.ctx.scene
    }

    /// Play `beat` at `time` on the Stage score and return its [`Span`].
    pub fn at(&mut self, time: impl CueTime, beat: impl Beat<StageCtx<'a>>) -> Span {
        beat.play(&mut self.ctx, time.cue_nanos())
    }

    /// Play `beat` at `time` and record a named [`crate::plan::CuePlan`] for its span.
    pub fn cue(
        &mut self,
        id: impl Into<String>,
        time: impl CueTime,
        beat: impl Beat<StageCtx<'a>>,
    ) -> Span {
        let span = self.at(time, beat);
        self.ctx.scene.cue(id, span.start, span.end);
        span
    }
}

impl StageActor {
    /// Open a composable [`StageScore`] over this stage and `scene`.
    pub fn score<'a>(&'a mut self, scene: &'a mut PlanBuilder) -> StageScore<'a> {
        StageScore::new(self, scene)
    }
}

impl PlanBuilder {
    /// Open a composable [`Score`] over this plan builder.
    pub fn score(&mut self) -> Score<'_, Self> {
        Score::new(self)
    }
}

/// Composable [`Beat`] constructors for [`StageActor`] and [`StageScore`].
pub mod stage {
    use super::*;

    /// Declare `property` with `initial` value if not already declared (impulse).
    pub fn channel<'a>(property: impl Into<String>, initial: f32) -> impl Beat<StageCtx<'a>> {
        let property = property.into();
        impulse(move |ctx: &mut StageCtx<'a>, _at| {
            ctx.stage.channel(ctx.scene, &property, initial);
        })
    }

    /// Declare multiple `(property, initial)` channels in order (impulse).
    pub fn channels<'a, I, S>(pairs: I) -> impl Beat<StageCtx<'a>>
    where
        I: IntoIterator<Item = (S, f32)>,
        S: AsRef<str>,
    {
        impulse(move |ctx: &mut StageCtx<'a>, _at| {
            for (property, initial) in pairs {
                ctx.stage.channel(ctx.scene, property.as_ref(), initial);
            }
        })
    }

    /// Jump `property` to `value` at the cue time (impulse).
    pub fn set<'a>(property: impl Into<String>, value: f32) -> impl Beat<StageCtx<'a>> {
        let property = property.into();
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage.set(ctx.scene, &property, at, value);
        })
    }

    /// Critically-damped spring on `property` to `target` over `seconds` (impulse).
    pub fn to<'a>(
        property: impl Into<String>,
        target: f32,
        seconds: f32,
    ) -> impl Beat<StageCtx<'a>> {
        let property = property.into();
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage.to(ctx.scene, &property, at, target, seconds);
        })
    }

    /// Spring `property` to `target` with `seconds` and `bounce` (impulse).
    pub fn bounce<'a>(
        property: impl Into<String>,
        target: f32,
        seconds: f32,
        bounce: f32,
    ) -> impl Beat<StageCtx<'a>> {
        let property = property.into();
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage
                .bounce(ctx.scene, &property, at, target, seconds, bounce);
        })
    }

    /// Spring `property` to `target` with a named [`SpringPlan`] feel (impulse).
    pub fn spring<'a>(
        property: impl Into<String>,
        target: f32,
        feel: SpringPlan,
    ) -> impl Beat<StageCtx<'a>> {
        let property = property.into();
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            let ch = ctx.stage.channel(ctx.scene, &property, 0.0);
            ctx.scene.spring_with(&ch, at, target, feel);
        })
    }

    /// Ease `property` to `target` over `seconds` along `curve` (impulse).
    pub fn ease<'a>(
        property: impl Into<String>,
        target: f32,
        seconds: f32,
        curve: Ease,
    ) -> impl Beat<StageCtx<'a>> {
        let property = property.into();
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage
                .ease(ctx.scene, &property, at, target, seconds, curve);
        })
    }

    /// Start an elapsed-seconds clock on `property` running to the end of the scene (impulse).
    pub fn clock<'a>(property: impl Into<String>) -> impl Beat<StageCtx<'a>> {
        let property = property.into();
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage.clock(ctx.scene, &property, at);
        })
    }

    /// Start a fixed-lifetime elapsed-seconds clock on `property` for `seconds` (impulse).
    pub fn clock_for<'a>(property: impl Into<String>, seconds: f32) -> impl Beat<StageCtx<'a>> {
        let property = property.into();
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage.clock_for(ctx.scene, &property, at, seconds);
        })
    }

    /// Instant-attack flash to `peak` followed by a cubic-out decay to `rest` (impulse).
    pub fn hit<'a>(property: impl Into<String>, peak: f32, rest: f32) -> impl Beat<StageCtx<'a>> {
        let property = property.into();
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage.hit(ctx.scene, &property, at, peak, rest);
        })
    }

    /// Two-frame shove on `[x, y]` that springs back past rest (impulse).
    pub fn kick<'a>(channels: [&'a str; 2], offset: [f32; 2]) -> impl Beat<StageCtx<'a>> {
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage.kick(ctx.scene, channels, at, offset);
        })
    }

    /// Camera impact jolt along `direction` with `strength` (impulse).
    pub fn jolt<'a>(direction: [f32; 2], strength: f32) -> impl Beat<StageCtx<'a>> {
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage.jolt(ctx.scene, at, direction, strength);
        })
    }

    /// Settle `card` into place; spans the `500 ms` until the panel is ready to wire.
    pub fn settle_in<'a>(card: impl Into<String>) -> impl Beat<StageCtx<'a>> {
        let card = card.into();
        action(move |ctx: &mut StageCtx<'a>, at| ctx.stage.settle_in(ctx.scene, &card, at))
    }

    /// Type `label` in at `chars_per_second`; spans until the last character appears.
    pub fn type_in<'a>(label: impl Into<String>, chars_per_second: f32) -> impl Beat<StageCtx<'a>> {
        let label = label.into();
        action(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage.type_in(ctx.scene, &label, at, chars_per_second)
        })
    }

    /// Draw `beam` over `seconds` (after the port pop); spans until contact at the far end.
    pub fn connect<'a>(beam: impl Into<String>, seconds: f32) -> impl Beat<StageCtx<'a>> {
        let beam = beam.into();
        action(move |ctx: &mut StageCtx<'a>, at| ctx.stage.connect(ctx.scene, &beam, at, seconds))
    }

    /// Back-timed connection that reaches its far port at the cue time `at`.
    pub fn connect_contacting<'a>(
        beam: impl Into<String>,
        seconds: f32,
    ) -> impl Beat<StageCtx<'a>> {
        let beam = beam.into();
        let lead = whole_millis(crate::stage::PORT_POP_SECONDS) + whole_millis(seconds);
        action(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage
                .connect(ctx.scene, &beam, at.saturating_sub(lead), seconds)
        })
    }

    /// Launch `packet` at the cue time and fly for `seconds`; spans `[at, arrival]`.
    pub fn send<'a>(packet: impl Into<String>, seconds: f32) -> impl Beat<StageCtx<'a>> {
        let packet = packet.into();
        action(move |ctx: &mut StageCtx<'a>, at| ctx.stage.send(ctx.scene, &packet, at, seconds))
    }

    /// Back-timed packet launch that *arrives* at the cue time `at` after flying
    /// for `seconds` (launching `seconds` earlier and gathering before that).
    ///
    /// Its span is `[at - flight, at]`, so chaining `.then(stage::land(...))`
    /// triggers the landing at `at`!
    pub fn send_arriving<'a>(
        packet_id: impl Into<String>,
        seconds: f32,
    ) -> impl Beat<StageCtx<'a>> {
        let packet_id = packet_id.into();
        let flight = whole_millis(seconds);
        move |ctx: &mut StageCtx<'a>, arrival: u64| {
            let launch = arrival.saturating_sub(flight);
            let landed = ctx.stage.send(ctx.scene, &packet_id, launch, seconds);
            let _ = packet::GATHER;
            Span::new(launch, landed)
        }
    }

    /// Pluck `beam` like a struck cable (impulse).
    pub fn twang<'a>(beam: impl Into<String>) -> impl Beat<StageCtx<'a>> {
        let beam = beam.into();
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage.twang(ctx.scene, &beam, at);
        })
    }

    /// Flash a card's ink or pulse an orb on arrival (impulse).
    pub fn land<'a>(element: impl Into<String>) -> impl Beat<StageCtx<'a>> {
        let element = element.into();
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.stage.land(ctx.scene, &element, at);
        })
    }

    /// Three-frame horizontal glitch burst on `card` (`27 ms` steps, then 0).
    /// Spans until the card returns to rest (`3 * 27 ms = 81 ms`).
    pub fn glitch<'a>(card: impl Into<String>, seeds: [f32; 3]) -> impl Beat<StageCtx<'a>> {
        let card = card.into();
        let step_nanos = crate::author::seconds(0.027);
        action(move |ctx: &mut StageCtx<'a>, at| {
            let prop = format!("{card}.glitch");
            let mut step = at;
            for seed in seeds.into_iter().chain([0.0]) {
                ctx.stage.set(ctx.scene, &prop, step, seed);
                step += step_nanos;
            }
            step - step_nanos
        })
    }

    /// Schedule a layer audio clip at the cue time (impulse).
    pub fn sound<'a>(
        id: impl Into<String>,
        path: impl Into<PathBuf>,
        duration_nanos: u64,
        gain_db: f32,
    ) -> impl Beat<StageCtx<'a>> {
        let id = id.into();
        let path = path.into();
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            ctx.scene.media(MediaPlan {
                id,
                path,
                kind: MediaKindPlan::Audio,
                role: MediaRolePlan::Layer,
                source_start_nanos: 0,
                source_end_nanos: duration_nanos,
                timeline_start_nanos: at,
                timeline_end_nanos: at + duration_nanos,
                gain_db,
            });
        })
    }

    /// Escape hatch: run any `&mut PlanBuilder` operation (such as an overlay
    /// actor's `.show` / `.hide` / `.type_in`) at the cue time as an impulse.
    pub fn on_scene<'a>(f: impl FnOnce(&mut PlanBuilder, u64) + 'a) -> impl Beat<StageCtx<'a>> {
        impulse(move |ctx: &mut StageCtx<'a>, at| {
            f(ctx.scene, at);
        })
    }
}

/// A suspicious same-timestamp multi-write on a continuous channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteConflict {
    pub channel_id: String,
    pub at_nanos: u64,
    pub count: usize,
}

/// Inspect a [`ScenePlan`] for accidental same-timestamp write collisions on any
/// continuous channel.
///
/// The single intentional same-timestamp compound idiom in Psychopomp is a
/// single `Set` immediately followed by a single `Spring` or `Ease` at the same
/// `at_nanos` (as in `settle_in`, `hit`, `clock_for`). Any other same-timestamp
/// group (such as two competing `Spring` writes from parallel branches, or a
/// `Set` after a `Spring` at the same timestamp) is returned as a [`WriteConflict`].
pub fn find_write_conflicts(plan: &ScenePlan) -> Vec<WriteConflict> {
    let mut conflicts = Vec::new();
    for channel in &plan.continuous_channels {
        let mut i = 0;
        while i < channel.events.len() {
            let at = channel.events[i].at_nanos();
            let mut j = i + 1;
            while j < channel.events.len() && channel.events[j].at_nanos() == at {
                j += 1;
            }
            let group = &channel.events[i..j];
            if group.len() > 1 {
                let valid_set_then_move = group.len() == 2
                    && matches!(group[0], TrackEventPlan::Set { .. })
                    && matches!(
                        group[1],
                        TrackEventPlan::Spring { .. } | TrackEventPlan::Ease { .. }
                    );
                if !valid_set_then_move {
                    conflicts.push(WriteConflict {
                        channel_id: channel.id.clone(),
                        at_nanos: at,
                        count: group.len(),
                    });
                }
            }
            i = j;
        }
    }
    conflicts
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{
        author::SECOND,
        plan::compile_channels,
        timeline::{PropertyId, Timeline},
    };

    /// Helper context for testing pure beat laws against `PlanBuilder`.
    struct TestEnv {
        scene: PlanBuilder,
        x: crate::author::ContinuousHandle,
        y: crate::author::ContinuousHandle,
        z: crate::author::ContinuousHandle,
    }

    impl TestEnv {
        fn new() -> Self {
            let mut scene = PlanBuilder::new("law-test", 20 * SECOND);
            let actor = scene
                .actor("a", "title-card", json!({"title": "T"}))
                .unwrap();
            let x = scene.continuous(&actor, "x", 0.0);
            let y = scene.continuous(&actor, "y", 0.0);
            let z = scene.continuous(&actor, "z", 0.0);
            Self { scene, x, y, z }
        }

        fn run(beat: impl Beat<Self>, at: u64) -> (Span, serde_json::Value, Timeline) {
            let mut env = Self::new();
            let span = beat.play(&mut env, at);
            let plan = env.scene.finish().unwrap();
            let timeline = compile_channels(
                plan.continuous_channels
                    .iter()
                    .map(|c| (c, PropertyId::new(&c.id))),
                plan.duration_nanos,
                |s| match s {
                    crate::plan::ScalarPlan::Literal(v) => Ok(*v),
                    _ => unreachable!(),
                },
            )
            .unwrap();
            let json = serde_json::to_value(&plan).unwrap();
            (span, json, timeline)
        }
    }

    fn spring_x(target: f32, dur: u64) -> impl Beat<TestEnv> + Clone {
        move |env: &mut TestEnv, at: u64| {
            env.scene.spring(&env.x, at, target, 0.4, 0.1);
            Span::new(at, at + dur)
        }
    }

    fn spring_y(target: f32, dur: u64) -> impl Beat<TestEnv> + Clone {
        move |env: &mut TestEnv, at: u64| {
            env.scene.spring(&env.y, at, target, 0.5, 0.0);
            Span::new(at, at + dur)
        }
    }

    fn ease_z(target: f32, seconds: f32) -> impl Beat<TestEnv> + Clone {
        let dur = whole_millis(seconds);
        move |env: &mut TestEnv, at: u64| {
            env.scene
                .ease(&env.z, at, target, seconds, Ease::Smootherstep);
            Span::new(at, at + dur)
        }
    }

    fn impulse_x(target: f32) -> impl Beat<TestEnv> + Clone {
        move |env: &mut TestEnv, at: u64| {
            env.scene.spring(&env.x, at, target, 0.35, 0.0);
            Span::impulse(at)
        }
    }

    #[test]
    fn law_1_sequence_is_associative_and_has_empty_identity() {
        let a = spring_x(10.0, millis(300));
        let b = spring_y(25.0, millis(450));
        let c = ease_z(1.0, 0.6);

        let (span_l, plan_l, _) =
            TestEnv::run(a.clone().then(b.clone()).then(c.clone()), millis(100));
        let (span_r, plan_r, _) = TestEnv::run(a.clone().then(b.then(c)), millis(100));
        assert_eq!(span_l, span_r);
        assert_eq!(plan_l, plan_r);

        let (span_id_l, plan_id_l, _) = TestEnv::run(empty().then(a.clone()), millis(200));
        let (span_id_r, plan_id_r, _) = TestEnv::run(a.clone().then(empty()), millis(200));
        let (span_base, plan_base, _) = TestEnv::run(a, millis(200));
        assert_eq!(span_id_l, span_base);
        assert_eq!(span_id_r, span_base);
        assert_eq!(plan_id_l, plan_base);
        assert_eq!(plan_id_r, plan_base);
    }

    #[test]
    fn law_2_hold_fusion_and_delay_isomorphism_are_exact_in_nanoseconds() {
        let a = spring_x(50.0, millis(400));
        let d1 = 100_000_000;
        let d2 = 200_000_000;

        // hold(d1).then(hold(d2)).then(a) == hold(d1 + d2).then(a) == a.after(d1 + d2)
        let (s1, p1, _) = TestEnv::run(hold(d1).then(hold(d2)).then(a.clone()), 0);
        let (s2, p2, _) = TestEnv::run(hold(d1 + d2).then(a.clone()), 0);
        let (s3, p3, _) = TestEnv::run(a.clone().after(d1).after(d2), 0);
        let (s4, p4, _) = TestEnv::run(a.after(d1 + d2), 0);

        assert_eq!(s1, s2);
        assert_eq!(s2, s3);
        assert_eq!(s3, s4);
        assert_eq!(p1, p2);
        assert_eq!(p2, p3);
        assert_eq!(p3, p4);
    }

    #[test]
    fn law_3_delay_shifts_into_sequence_head() {
        let a = spring_x(12.0, millis(250));
        let b = spring_y(-8.0, millis(600));
        let d = millis(350);

        // (a.then(b)).after(d) == a.after(d).then(b)
        let (s1, p1, _) = TestEnv::run(a.clone().then(b.clone()).after(d), millis(50));
        let (s2, p2, _) = TestEnv::run(a.after(d).then(b), millis(50));
        assert_eq!(s1, s2);
        assert_eq!(p1, p2);
    }

    #[test]
    fn law_4_parallel_is_associative_and_distributes_delay() {
        let a = spring_x(10.0, millis(300));
        let b = spring_y(20.0, millis(700));
        let c = ease_z(0.5, 0.4);
        let d = millis(180);

        // Associativity
        let (s_l, p_l, _) = TestEnv::run(a.clone().also(b.clone()).also(c.clone()), millis(50));
        let (s_r, p_r, _) = TestEnv::run(a.clone().also(b.clone().also(c)), millis(50));
        assert_eq!(s_l, s_r);
        assert_eq!(p_l, p_r);

        // Delay distributivity: (a.also(b)).after(d) == a.after(d).also(b.after(d))
        let (sd_1, pd_1, _) = TestEnv::run(a.clone().also(b.clone()).after(d), millis(100));
        let (sd_2, pd_2, _) = TestEnv::run(a.after(d).also(b.after(d)), millis(100));
        assert_eq!(sd_1, sd_2);
        assert_eq!(pd_1, pd_2);
    }

    #[test]
    fn law_5_impulse_absorption_and_accompaniment_laws() {
        let p = impulse_x(42.0);
        let a = spring_y(100.0, millis(500));

        // Impulse at head of sequence is identical to parallel: p.then(a) == p.also(a)
        let (s_seq, plan_seq, _) = TestEnv::run(p.clone().then(a.clone()), millis(120));
        let (s_par, plan_par, _) = TestEnv::run(p.clone().also(a.clone()), millis(120));
        assert_eq!(s_seq, s_par);
        assert_eq!(plan_seq, plan_par);

        // `a.with(b)` preserves `a`'s span even when `b` is longer
        let long_bg = ease_z(1.0, 2.0);
        let (s_with, _, _) = TestEnv::run(a.clone().with(long_bg), millis(100));
        assert_eq!(s_with, Span::new(millis(100), millis(600)));

        // `a.on_end(p)` runs impulse `p` at `a.end` and preserves `a`'s span
        let (s_end, plan_end, _) = TestEnv::run(a.clone().on_end(p.clone()), millis(100));
        let (s_then, plan_then, _) = TestEnv::run(a.then(p), millis(100));
        assert_eq!(s_end, s_then);
        assert_eq!(plan_end, plan_then);
    }

    #[test]
    fn law_6_stagger_equals_delayed_parallel_expansion() {
        let gap = millis(120);
        let targets = [10.0_f32, 30.0, 60.0, 90.0];

        let (s_stag, p_stag, _) =
            TestEnv::run(stagger(gap, targets, |t| spring_x(t, millis(200))), SECOND);
        let (s_manual, p_manual, _) = TestEnv::run(
            all![
                spring_x(10.0, millis(200)).after(0),
                spring_x(30.0, millis(200)).after(gap),
                spring_x(60.0, millis(200)).after(2 * gap),
                spring_x(90.0, millis(200)).after(3 * gap),
            ],
            SECOND,
        );
        assert_eq!(s_stag, s_manual);
        assert_eq!(p_stag, p_manual);
    }

    #[test]
    fn law_7_retargeted_parallel_springs_commute_across_distinct_timestamps() {
        // Three writes to the SAME channel `x` at distinct offsets (0ms, 180ms, 420ms).
        // Regardless of parallel branch order, the compiled Timeline must sample
        // bit-identically (position and velocity) at every timestamp.
        let w0 = impulse_x(100.0);
        let w1 = impulse_x(-40.0).after(millis(180));
        let w2 = impulse_x(25.0).after(millis(420));

        let (_, _, tl_forward) =
            TestEnv::run(all![w0.clone(), w1.clone(), w2.clone()], millis(100));
        let (_, _, tl_reverse) = TestEnv::run(all![w2, w0, w1], millis(100));

        let prop = PropertyId::new("a.x");
        for step in 0..=200 {
            let t = step as f64 * 0.01;
            let s_f = tl_forward.sample_at(&prop, t).unwrap();
            let s_r = tl_reverse.sample_at(&prop, t).unwrap();
            assert_eq!(
                [s_f.position.to_bits(), s_f.velocity.to_bits()],
                [s_r.position.to_bits(), s_r.velocity.to_bits()],
                "mismatch at t={t:.2}s"
            );
        }
        // Verify velocity is genuinely carried across the retarget at t = 0.28s (100ms + 180ms)
        let at_retarget = tl_forward.sample_at(&prop, 0.28).unwrap();
        assert!(at_retarget.velocity.abs() > 10.0);
    }

    /// Random beat AST for property-testing algebraic rewrites.
    #[derive(Clone, Debug)]
    enum Expr {
        Empty,
        Hold(u64),
        Leaf { channel: u8, target: f32, span: u64 },
        After(u64, Box<Expr>),
        Seq(Box<Expr>, Box<Expr>),
        Par(Box<Expr>, Box<Expr>),
    }

    impl Expr {
        fn generate(seed: &mut u64, depth: u32) -> Self {
            let mut next = || {
                *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                *seed
            };
            if depth == 0 {
                return match next() % 3 {
                    0 => Self::Empty,
                    1 => Self::Hold((next() % 5 + 1) * millis(40)),
                    _ => Self::Leaf {
                        channel: (next() % 3) as u8,
                        target: ((next() % 200) as f32) - 50.0,
                        span: (next() % 4) * millis(50),
                    },
                };
            }
            match next() % 5 {
                0 => Self::Empty,
                1 => Self::Hold((next() % 5 + 1) * millis(30)),
                2 => Self::After(
                    (next() % 4) * millis(45),
                    Box::new(Self::generate(seed, depth - 1)),
                ),
                3 => Self::Seq(
                    Box::new(Self::generate(seed, depth - 1)),
                    Box::new(Self::generate(seed, depth - 1)),
                ),
                _ => Self::Par(
                    Box::new(Self::generate(seed, depth - 1)),
                    Box::new(Self::generate(seed, depth - 1)),
                ),
            }
        }

        /// Apply algebraic equivalences (associativity, identity, hold-delay isomorphism,
        /// delay distribution over Par, delay shift into Seq) to produce a structurally
        /// different but denotationally equivalent AST.
        fn rewrite(&self) -> Self {
            match self {
                Self::Empty | Self::Hold(_) | Self::Leaf { .. } => self.clone(),
                Self::After(0, inner) => inner.rewrite(),
                Self::After(d1, inner) => match inner.as_ref() {
                    Self::After(d2, sub) => Self::After(d1 + d2, Box::new(sub.rewrite())),
                    Self::Par(a, b) => Self::Par(
                        Box::new(Self::After(*d1, a.clone()).rewrite()),
                        Box::new(Self::After(*d1, b.clone()).rewrite()),
                    ),
                    Self::Seq(a, b) => Self::Seq(
                        Box::new(Self::After(*d1, a.clone()).rewrite()),
                        Box::new(b.rewrite()),
                    ),
                    _ => Self::Seq(Box::new(Self::Hold(*d1)), Box::new(inner.rewrite())),
                },
                Self::Seq(a, b) => {
                    let a = a.rewrite();
                    let b = b.rewrite();
                    match (a, b) {
                        (Self::Empty, b) => b,
                        (a, Self::Empty) => a,
                        (Self::Hold(d1), Self::Hold(d2)) => Self::Hold(d1 + d2),
                        (Self::Seq(x, y), z) => {
                            Self::Seq(x, Box::new(Self::Seq(y, Box::new(z)).rewrite()))
                        }
                        (a, b) => Self::Seq(Box::new(a), Box::new(b)),
                    }
                }
                Self::Par(a, b) => {
                    let a = a.rewrite();
                    let b = b.rewrite();
                    match (a, b) {
                        (Self::Empty, b) => b,
                        (a, Self::Empty) => a,
                        (Self::Par(x, y), z) => {
                            Self::Par(x, Box::new(Self::Par(y, Box::new(z)).rewrite()))
                        }
                        (a, b) => Self::Par(Box::new(a), Box::new(b)),
                    }
                }
            }
        }
    }

    impl Beat<TestEnv> for &Expr {
        fn play(self, env: &mut TestEnv, at: u64) -> Span {
            match self {
                Expr::Empty => empty().play(env, at),
                Expr::Hold(d) => hold(*d).play(env, at),
                Expr::Leaf {
                    channel,
                    target,
                    span,
                } => {
                    let handle = match channel {
                        0 => &env.x,
                        1 => &env.y,
                        _ => &env.z,
                    };
                    env.scene.spring(handle, at, *target, 0.35, 0.0);
                    Span::new(at, at + *span)
                }
                Expr::After(d, inner) => inner.as_ref().after(*d).play(env, at),
                Expr::Seq(a, b) => a.as_ref().then(b.as_ref()).play(env, at),
                Expr::Par(a, b) => a.as_ref().also(b.as_ref()).play(env, at),
            }
        }
    }

    #[test]
    fn property_200_random_beat_trees_preserve_span_events_and_sampled_trajectories_under_law_rewrites()
     {
        let mut seed = 0x5EED_CAFE_BAAD_F00Du64;
        for case in 0..200 {
            let tree = Expr::generate(&mut seed, 4);
            let rewritten = tree.rewrite();
            let start = (case as u64 % 5) * millis(50);

            let (span_1, plan_1, tl_1) = TestEnv::run(&tree, start);
            let (span_2, plan_2, tl_2) = TestEnv::run(&rewritten, start);

            assert_eq!(span_1, span_2, "span mismatch on random tree #{case}");
            assert_eq!(
                plan_1["continuousChannels"], plan_2["continuousChannels"],
                "compiled channels mismatch on random tree #{case}"
            );

            for prop in ["a.x", "a.y", "a.z"] {
                let id = PropertyId::new(prop);
                for sample in [0.0, 0.15, 0.33, 0.75, 1.25] {
                    assert_eq!(
                        tl_1.sample_at(&id, sample),
                        tl_2.sample_at(&id, sample),
                        "sampled trajectory mismatch on tree #{case} at {sample}"
                    );
                }
            }
        }
    }

    #[test]
    fn conflict_detector_allows_set_then_spring_and_flags_competing_parallel_springs() {
        let mut env = TestEnv::new();
        // Valid idiom: set then spring at same timestamp (like `settle_in` or `hit`)
        env.scene.set(&env.x, SECOND, 16.0);
        env.scene.spring(&env.x, SECOND, 0.0, 0.55, 0.16);
        let clean = env.scene.finish().unwrap();
        assert!(find_write_conflicts(&clean).is_empty());

        // Invalid collision: two parallel springs on `y` at the same timestamp
        let mut env2 = TestEnv::new();
        let collision = spring_y(10.0, 0).also(spring_y(20.0, 0));
        collision.play(&mut env2, SECOND);
        let dirty = env2.scene.finish().unwrap();
        assert_eq!(
            find_write_conflicts(&dirty),
            vec![WriteConflict {
                channel_id: "a.y".into(),
                at_nanos: SECOND,
                count: 2,
            }]
        );
    }
}
