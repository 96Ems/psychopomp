# Kinograph Domain Language

## Code Document

The complete set of code lines that may appear in a scene. Every line has a stable line ID and styled spans.

## Code Snapshot

An ordered list of stable line IDs describing one meaningful state of a code document. A snapshot contains state, not motion.

## Stable Line

A code line whose identity survives between snapshots. Its screen position may move when surrounding lines enter or leave, but its text object is not replaced.

## Code Transition

The compiled relationship between two code snapshots. Sampling a code transition places every stable, entering, and exiting line at an arbitrary progress value.

## Code Edit

A renderer-independent actor for one coordinated structural change to code. A Code Edit owns layout and content progress tracks, supplies their zero-state initial values, enters or exits through one Motion, and samples a Code Transition from a compiled Scene. Scenes that deliberately stagger layout and content may still animate those tracks independently.

## Motion State

The position and velocity of one animated scalar at a specific time. Carrying both values allows a later trajectory to preserve momentum.

## Animation

A pure value describing a property change or the composition of other animations. Sequence, parallel, delay, and hold determine relative timing without rendering or mutating scene state.

## Scene

A pure Rust value containing initial property expressions and a composition. Compiling a scene resolves semantic targets and produces deterministic property tracks plus scheduled media placements and cue ranges.

## Scene Program

A lightweight Rust executable that may perform arbitrary calculations, imports, data loading, and control flow before emitting one Scene Plan. A Scene Program is durable authoring source; its emitted plan is compiled output.

## Scene Plan

A versioned, renderer-independent value containing stable actor declarations, continuous channels, state channels, exact cues, and media placements. Agents may inspect, validate, diff, and render a Scene Plan without recompiling or restarting the renderer.

## Scalar Plan

One compiled scalar source in a Scene Plan. It is either a finite literal or a reference to one component of a stable Semantic Target plus an offset. Renderer preparation resolves target references before the ordinary numeric Property Track is compiled.

## Continuous Channel

One named scalar property of a stable actor. Ordered set and spring events compile into a deterministic Property Track while preserving equal-time source order.

## State Channel

One named discrete property of a stable actor. Its compiled State Track retains the previous and current value, completed previous duration, transition time, and current age without depending on frame history.

## Renderer Recipe

A concrete rendering adapter selected by an actor declaration. Recipe payloads and pixels remain renderer-owned; the Scene Plan core validates identity and timing without understanding their visual implementation.

## Render Window

A positive exact range on the global scene clock selected for delivery. Output begins at time zero, intersecting media is trimmed and rebased, and visual sampling retains global time so ongoing trajectories do not restart.

## Composition

A pure, time-bearing value that arranges visual motion and media with sequence, parallel, delay, and hold. Composition owns cross-media timing; Motion remains responsible for visual property trajectories.

## Asset

Immutable source material identified independently from any use on the timeline. Audio, video, and image assets retain their original files while edits refer to them non-destructively.

## Clip

One positive-duration source range from an audio or video asset. Moving, copying, removing, or changing an audio clip's gain changes the edit without changing its source asset.

## Image Actor

A stable visual actor backed by an image asset. Position, scale, rotation, opacity, and blur are ordinary property tracks; unlike a clip, an image has no intrinsic timeline duration.

## Script Clip

A clip on the primary spoken-media track. Its transcript may drive structural edits, captions, and semantic timing.

## Layer Clip

Accompanying timed media such as music, sound effects, or B-roll. Layer clips share composition timing but do not implicitly become part of the editable transcript.

## Cue

A named timeline range. Cues may be authored around a composition or imported from transcript word and phrase timing; their start and end can synchronize motion and media. Scheduling a composition `at` a Cue places it at the Cue's start relative to the containing composition's origin, which is the shared media clock for root scene choreography.

## Transcript

An ordered set of words with source start and end times. Looking up a word occurrence produces a cue range on the same exact media clock used by script clips.

## Media Placement

A compiled relationship between a clip's immutable source range and its scheduled timeline range. Media placement time uses integer nanoseconds so edit boundaries remain exact across repeated composition.

## Property Track

The compiled trajectory of one scalar actor property. A later spring on the same track begins from the earlier trajectory's sampled position and velocity.

## Semantic Target

A stable named selection owned by an actor recipe, such as a logical range inside a stable code line. The plan core preserves identity and validates references; the renderer recipe measures concrete geometry. Highlights and pointers attach to Semantic Targets rather than authored screen coordinates.

## Inline Reveal

A transition that expands or collapses authored spans inside a Stable Line. Variable spans animate width, opacity, and blur while common prefix, infix, and suffix spans retain identity and move to their new positions without replacement. Opposing reveals can exchange slot alternatives horizontally while preserving maximum stability.

## Annotation

A transient or persistent visual attached to a semantic target without changing the target's identity. Short annotations are scheduled in a composition, resolve their semantic target during scene compilation, and sample a deterministic normalized phase at arbitrary media time. The lesson port demonstrates a persistent red error squiggle plus interchangeable prismatic-bloom and focus-pulse celebration effects.

## Pointer

A stable visual actor that directs attention to a semantic target. Its position and opacity are ordinary property tracks, so retargeting and temporal sampling use the same motion system as every other actor.

## Task

A stable visual actor representing one Effect computation. Composition schedules its idle, running, succeeded, failed, death, hidden, and retry state changes; the stable task ID preserves identity across those changes.

## Task State

One meaningful snapshot of a Task. A task state selects semantic content and visual targets, while the renderer derives the transition from the preceding state at arbitrary media time. A succeeded task may carry a result or represent payload-free completion.

## Task Pose

The renderer-independent position of one Task. Pose changes are composition leaves distinct from semantic Task State changes, so row recentering cannot replay state feedback. Compiled x/y property tracks preserve position and velocity when layout movement is interrupted or redirected.

## Terminal Recording

An immutable recording of a real terminal interaction used as source pixels inside authored choreography. Kinograph samples the recording by media time, while a terminal frame supplies presentation such as rounded clipping, whole-card camera motion, explanatory split views, and effects. The recording remains evidence of the actual product behavior rather than a reconstructed terminal simulation.

## Temporal Sample

One evaluation of the complete scene within an output frame's shutter interval. Kinograph averages temporal samples to produce motion blur from real scene movement.

## Editor Frame

The renderer-neutral description of one sampled editor scene: panel position, focus state, and placed code lines.

## Published Lesson

An immutable Effect Institute section artifact containing canonical narration, word timing, stable code template identities, step frames, and optional component snapshots. A Published Lesson is imported into ordinary Kinograph tracks and renderer recipes; it is not a second authoring language.

## Chapter Reel

One encoded video that arranges Published Lessons in manifest order with chapter and group title intervals. Every section retains its own local media clock and actor namespace while narration clips are placed exactly on the chapter clock.
