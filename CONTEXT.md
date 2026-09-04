# Kinograph Domain Language

## Code Document

The complete set of code lines that may appear in a scene. Every line has a stable line ID and styled spans.

## Code Snapshot

An ordered list of stable line IDs describing one meaningful state of a code document. A snapshot contains state, not motion.

## Stable Line

A code line whose identity survives between snapshots. Its screen position may move when surrounding lines enter or leave, but its text object is not replaced.

## Maximum Stability

The authoring and rendering contract that preserves common code identity across step destinations. Only changed content enters, exits, or is replaced. Retained lines and inline parts may move to accommodate changed layout, but do not unnecessarily disappear, reappear, fade, or blur. Stability concerns the smallest meaningful content delta, not keeping every screen coordinate fixed.

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

## UI Snapshot

One complete meaningful state of a simulated interface recipe. A UI Snapshot contains ordered recipe-local item identities and semantic values, not rectangles or animation. Reordering an item changes its layout target without replacing its identity.

## Keyed Layout Transition

A renderer-owned compilation from timestamped UI Snapshots into arbitrary-time position and presence tracks for stable recipe-local item IDs. Each retarget begins from the earlier trajectory's sampled position and velocity. Layout supplies target positions; the Keyed Layout Transition supplies motion between them.

## UI Surface

One stable actor rendered as a simulated application window, dashboard, or panel by a concrete Renderer Recipe. Internal controls and rows remain recipe-local unless another actor demonstrates a need to address them through a Semantic Target.

## Keyed Grid

A finite product of three immutable ordered value axes. Each tuple has stable recipe-local identity, independent of its visibility or arrangement. A row or table can show a slice of that catalog; revealing another axis value adds visible tuples without replacing retained ones.

## Grid Snapshot

The visible prefix of each axis, an Arrangement, and optional focused depth slice. Straight-on and angled orthographic arrangements view the same connected 3D geometry; a flat-looking grid can still contain depth. Reassociated arrangements regroup the tuples. Layout supplies position, extent, and cutaway targets; ordinary Property Tracks supply motion. Extents reveal a connected grid without scaling individual cells; the projection deliberately centers its currently sampled visible bounds during growth. A focused slice clips away other layers without deleting their identities. Reassociation changes `((a, b), c)` into `(a, (b, c))` without adding or losing tuples. Growing a product is not an isomorphism between the smaller and larger sets.

A cell's primary symbol and secondary label are representations of its tuple,
not its identity. Row, column, and depth headings describe the values along the
edges and remain separate from the cell labels.

## Renderer Recipe

A concrete rendering adapter selected by an actor declaration. Recipe payloads and pixels remain renderer-owned; the Scene Plan core validates identity and timing without understanding their visual implementation.

## Render Window

A positive exact range on the global scene clock selected for delivery. Output begins at time zero, intersecting media is trimmed and rebased, and visual sampling retains global time so ongoing trajectories do not restart.

## Presentation Step

An explicitly ordered destination in a Scene Plan, with a stable ID, title, entry start, and held endpoint on the authored scene clock. Entry start may equal the held endpoint for a still step. Unlike a Cue, a Presentation Step is a navigation destination, and step entry ranges cannot overlap. The native player derives scalar target poses at held endpoints, then animates between those destinations on its own pausable clock. Entry start supplies the explicit Replay pose. Authors choose meaningful endpoint poses; live waiting never alters the automatic video schedule.

## Presentation Deck

An ordered collection of titled Scene Plans. Each slide owns its Presentation Steps and local Playback clock. Changing slides preserves the selected step and pauses the departed slide; returning resumes only motion that was running when it was left. Slide navigation is separate from step navigation.

## Playback

Interactive navigation among Presentation Steps. Next and Previous retarget continuous channels from their sampled position and velocity through the same Property Track compiler used for video. Unchanged channel destinations retain their trajectories. Pause freezes the local clock without losing motion state; Replay deliberately restarts from an entry pose. Playback retains an immutable compiled timeline for each navigation revision, so late rendered frames cannot change the current destination. It is not reverse playback of a movie.

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

A spring's settling time is deterministic. After that time the segment stays exactly at rest unless another segment retargets it; starting an unrelated channel cannot wake it.

## Semantic Target

A stable named selection owned by an actor recipe, such as a logical range inside a stable code line. The plan core preserves identity and validates references; the renderer recipe measures concrete geometry. Highlights and pointers attach to Semantic Targets rather than authored screen coordinates.

Attachment follows sampled visible layout, including collapsed Inline Reveals and moving Stable Lines. Expanded text measurements are a preparation baseline, not final coordinates for every step.

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

Planned interactive Tasks lower state changes into continuous geometry and content-presence tracks. A running Task can keep the local playback clock active after those tracks settle, while pause and reduced motion still stop its ambient animation. It illustrates a computation rather than executing an actual Effect.

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
