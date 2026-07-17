# Animation Prior Art

Kinograph should not invent its authoring and timeline model without pressure-testing it against established animation systems. This document records the parts worth borrowing and the constraints that make direct adoption insufficient.

## Current Design Question

The immediate question is how edited narration, transcript cues, and deterministic visual motion compose on one inspectable media clock. A graphical timeline remains deferred.

The API should eventually support both:

- relative choreography: sequence, parallel, delay, stagger, and reusable animation procedures
- explicit timing: keyframes or segments placed at known times

Both forms must compile into trajectories that can be sampled independently at arbitrary timestamps. Rendering frame 120 directly must match frame 120 from a complete render.

## Manim

[Manim](https://docs.manim.community/en/stable/) is the strongest reference for semantic scene construction.

Relevant ideas:

- `Scene.play` makes choreography read as a sequence of meaningful operations.
- `AnimationGroup`, `Succession`, and `LaggedStart` compose parallel, sequential, and staggered work.
- Animations expose normalized progress through `interpolate(alpha)`.
- Mobjects preserve visual identity while transforms act on them.
- Trackers and updaters let one animated value drive dependent geometry.

What Kinograph should borrow:

- animations as composable values rather than scattered property calculations
- first-class sequence, parallel, and stagger composition
- actor-targeted semantic operations such as enter, move, focus, and transform
- normalized sampling beneath a readable imperative authoring surface

What Kinograph should avoid:

- frame-order-dependent mutation as the source of truth
- unrestricted per-frame callbacks that cannot be serialized or sampled out of order
- requiring authors to understand renderer objects to express common choreography

## Motion Canvas

[Motion Canvas](https://motioncanvas.io/docs/) is the closest TypeScript reference for code-authored motion graphics.

Relevant ideas:

- generator functions make sequential choreography linear and readable
- yielded animation generators compose through flow helpers
- signals serve as values, setters, derived values, and tween constructors
- a property can be assigned immediately or animated over a duration through one coherent interface
- dependencies can derive layout or geometry from animated signals

What Kinograph should borrow:

- generator-style choreography as a candidate authoring frontend
- one composition algebra shared by waits, tweens, springs, sequences, and parallel groups
- typed animatable properties and derived values
- reusable procedures that return animation values

What Kinograph should test carefully:

- whether generator execution can compile once into a durable timeline instead of becoming runtime mutable state
- whether overloaded signal getter/setter/tween syntax remains understandable for a serializable scene compiler
- how interrupted springs preserve velocity when a later operation retargets the same property

## Remotion

[Remotion](https://www.remotion.dev/docs/) is the strongest reference for deterministic frame-addressed evaluation.

Relevant ideas:

- the current frame is explicit input to rendering
- `Sequence` shifts local time and nested sequences compose offsets
- `Series` expresses consecutive ranges without manual arithmetic
- `interpolate` maps a sampled driver across keyframes
- `spring` is a pure function of frame and configuration

What Kinograph should borrow:

- rendering as a pure function of composition time
- local time domains for nested clips and reusable components
- explicit trim, delay, and duration semantics
- interpolation as a separate operation from the source driver

What Kinograph should improve:

- authors should not routinely calculate frame numbers
- time should use seconds or typed durations and remain independent of output frame rate
- interrupted physical motion should preserve velocity rather than restart from a newly sampled position
- layout, camera, and actor movement should all participate in temporal sampling

## Motion And React Motion

[Motion](https://motion.dev/) (formerly Framer Motion) and the older [React Motion](https://github.com/chenglou/react-motion) are references for target-driven animation.

Relevant ideas:

- authors state destinations rather than manually generating intermediate frames
- keyframes and springs share a property-oriented interface
- variants, stagger, and timelines orchestrate related actors
- layout changes can become motion while preserving element identity
- React Motion emphasizes spring destinations and natural interruption over fixed-duration curves
- Motion's `visualDuration` maps to angular frequency `2π / (visualDuration × 1.2)`; `bounce: 0` is critically damped

What Kinograph should borrow:

- target-driven property animation
- physical interruption semantics
- ergonomic defaults and named motion profiles
- separation between stable actor identity and changing target state

What Kinograph should avoid:

- dependence on browser layout or DOM lifecycle
- implicit real-time state that makes offline random-access sampling ambiguous
- APIs where convenience hides the compiled timeline and prevents inspection

## GPUI

[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) is prior art for composing high-performance Rust interfaces from plain values. Its `RenderOnce`, `IntoElement`, and `Element` layers rebuild declarative element structure while retaining application state outside the tree; layout, prepaint, and paint remain distinct phases.

What Kinograph should borrow:

- plain data components whose rendering can be rebuilt from one sampled state
- stable identity only where state, caching, or layout animation requires it
- explicit layout before paint
- small fluent fixed-canvas layout operations
- cached shaped text and expensive immutable surfaces
- call-order painting and lexical clipping for concrete renderer recipes

What Kinograph should avoid:

- reactive entities, notifications, subscriptions, and invalidation as animation truth
- wall-clock animation elements that request future frames
- input dispatch, hitboxes, focus, accessibility, and platform-window lifecycle
- a public retained element tree or serialized flexbox vocabulary
- copying GPUI's runtime state machinery into an offline renderer whose complete state is already a function of media time

The `deployment-queue` proof takes this narrower path. One UI Snapshot produces recipe-owned target layout, stable keyed tracks preserve motion across snapshot changes, and a private immediate-mode painter reconstructs the complete UI Surface at every Temporal Sample.

## Animations.dev

[Animations.dev](https://animations.dev/learn) by Emil Kowalski is practical prior art for judging product motion rather than merely implementing it. Its animation-theory, Family drawer, Dynamic Island, and good-versus-great lessons reinforce several rules that apply directly to Kinograph scenes:

- record the result and inspect state changes frame by frame instead of tuning only from code
- give every animation one legible purpose; repeated decoration reduces the value of motion
- use decelerating motion for entry and restrained ease-in-out or no-bounce springs for persistent layout changes
- default product springs to no bounce unless a gesture or physical collision supplies the force that justifies it
- preserve velocity when an in-flight layout motion is redirected
- scale perceptual duration with element size and distance while keeping frequently encountered feedback short
- keep exits simpler than entries and avoid large scale changes; a value near the destination reads as continuity rather than creation from nothing
- preserve spatial consistency, including a stable direction and a single meaningful destination for reordering
- bridge rich state replacement deliberately: crossfade simple content, or sequence outgoing and incoming content when showing both creates visual collisions
- choose easing before duration, then inspect the result in slow motion and at normal speed
- prefer solid materials or eased multi-stop gradients; plain two-stop color gradients expose banding and often add hierarchy-free decoration

The deployment queue applies the relevant subset with solid materials, stable service order, a no-bounce entrance, critically damped position and presence springs, previous-to-current phase presentation, staggered rich-state replacement, and event-boundary artifact checks. Separate keyed-layout tests prove velocity-preserving redirection and exit/re-entry behavior; the canonical scene does not claim to demonstrate those motions or size-dependent timing. Kinograph still compiles these choices into deterministic arbitrary-time tracks rather than adopting a browser animation runtime.

### Pointer Motion Principles

[Emil Kowalski's animation guidance](https://emilkowal.ski/ui/great-animations) emphasizes natural spring motion, speed, purpose, interruptibility, and reviewing work in slow motion or frame by frame. His published design-engineering skill specifically recommends spring interpolation for decorative pointer-following motion because direct target assignment feels artificial.

Kinograph applies that guidance with restraint:

- the pointer is explanatory rather than a frequently repeated control
- translation remains fast, interruptible, and velocity-preserving
- the pointer uses the filled Phosphor `HandPointingIcon` style that `effect-institute` selects by default
- acceleration makes the cursor lean against a direction change, creating physically grounded anticipation
- velocity turns the cursor into travel while deceleration carries it through the arrival
- pointer targets and highlight targets remain independent so attention can lead or leave the highlighted concept
- every pointer transform participates in temporal sampling and motion blur

Anticipation and follow-through come from classical animation, but should remain secondary action here. They must clarify direction and weight without delaying the pointer or turning functional explanation into decorative spectacle.

## Theatre.js

[Theatre.js](https://www.theatrejs.com/) is prior art for explicit keyframe authoring and timeline data.

Relevant ideas:

- sheets contain stable objects with typed animatable properties
- sequences hold keyframes and expose an explicit position
- code-defined objects can be driven by editor-authored timeline data
- a graph editor and dope sheet operate on the same property model used at runtime

What Kinograph should borrow now:

- a serializable property-track and keyframe model
- stable addresses for actors and their properties
- the principle that programmatic and visual authoring can target the same compiled representation

What Kinograph should defer:

- a graphical timeline, property inspector, graph editor, or extension system
- editor-specific project structures before the code-authored hero scene reveals the required data model

## Product Motion Guidance

[Apple's Human Interface Guidelines for Motion](https://developer.apple.com/design/human-interface-guidelines/motion), [Material 3's motion system](https://m3.material.io/styles/motion/overview/how-it-works), [IBM's classic animation principles](https://www.ibm.com/design/language/animation/classic-principles/), and [Fluent 2 choreography guidance](https://fluent2.microsoft.design/motion) constrain how classical animation principles should enter technical UI scenes.

Relevant ideas:

- feedback motion should be brief, precise, and immediately tied to a meaningful state change
- physics-based motion improves continuity and interruption, but visible oscillation should be reserved for expressive moments rather than every utility transition
- one primary action should establish the new state; glow, pulse, copy, and sound are subordinate actions that reinforce it
- success can use one restrained overshoot, failure should use a short directional impact and definitive rest, and terminal states should lose energy rather than behaving like a stronger recoverable failure
- running activity should remain readable without continuous decorative instability
- audio's main transient should coincide with visual contact, while longer sonic decay may provide follow-through after geometry has settled
- state must remain legible through shape, text, icon, and contrast without depending on motion, color, or sound alone

Kinograph applies these constraints to Effect Task states. Running uses compression and a directional energy sweep rather than perpetual shake. Success prioritizes result expansion and content resolution. Failure stages a short horizontal impact before its error bubble, then becomes still. Death darkens and settles with less scale instead of reusing failure shake. Layout-only changes do not restart semantic flashes or pulses.

## effect-institute

`/Users/kit/code/experiments/typescript/effect-institute` is local product prior art for semantic code states, not the immediate timeline model.

Relevant ideas:

- `line`, `stack`, `concat`, and `slot` preserve authored structure
- snapshots describe meaningful states instead of intermediate pixels
- stable line and part identity prevents unrelated code from being replaced
- focus and annotations target semantic content

Kinograph retains these concepts in its first lesson port. The published `effect-shows-errors` narration and word timing sidecar now drive ordinary actor properties and trajectories; inline slots lower to independent reveal properties on stable code lines.

## Working Synthesis

The likely architecture has three distinct levels:

1. Authors construct stable actors and compose semantic animations using sequence, parallel, delay, stagger, tween, spring, and set operations.
2. The frontend compiles those operations into explicit property tracks, segments, and keyframes in a versioned scene IR.
3. Rust samples the compiled trajectories at arbitrary times and renders the resulting scene state.

The authoring API may feel imperative, like Manim or Motion Canvas, while the compiled representation remains declarative and inspectable, like Theatre.js tracks evaluated with Remotion-style explicit time.

## Questions To Prototype

1. Can one small composition algebra express sequence, parallel, overlap, stagger, and delay without special cases?
2. Can a generator-style TypeScript scene compile deterministically without retaining mutable execution state at render time?
3. Can explicit keyframes and relative choreography compile into the same property-track representation?
4. When two operations target the same property, are overlap and interruption rules obvious and velocity-preserving?
5. Can every compiled track explain its source operation, resolved start time, duration, and motion profile?

The next API prototype should answer these questions with panel entry, camera reframing, code-state change, and focus intensity. It should not include narration synchronization or a visual timeline editor.
