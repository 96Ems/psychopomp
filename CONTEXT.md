# Psychopomp Domain Language

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

## Motion State

The position and velocity of one animated scalar at a specific time. Carrying both values allows a later trajectory to preserve momentum.

## Scene Program

A lightweight Rust executable that may perform arbitrary calculations, imports, data loading, and control flow before emitting one Scene Plan. A Scene Program is durable authoring source; its emitted plan is compiled output.

## Scene Plan

A versioned, renderer-independent value containing stable actor declarations, continuous channels, state channels, exact cues, and media placements. Agents may inspect, validate, diff, and render a Scene Plan without recompiling or restarting the renderer.

## Scalar Plan

One compiled scalar source in a Scene Plan. It is either a finite literal or a reference to one component of a stable Semantic Target plus an offset. Renderer preparation resolves target references before the ordinary numeric Property Track is compiled.

## Continuous Channel

One named scalar property of a stable actor. Ordered set, spring, and ease events compile into a deterministic Property Track while preserving equal-time source order. An ease moves from the current value to a target along a named curve over an exact duration, carrying the curve's velocity so a later spring continues without a jump.

## State Channel

One named discrete property of a stable actor. Its compiled State Track retains the previous and current value, completed previous duration, transition time, and current age without depending on frame history.

## Keyed Grid

A finite product of three immutable ordered value axes. Each tuple has stable recipe-local identity, independent of its visibility or arrangement. A row or table can show a slice of that catalog; revealing another axis value adds visible tuples without replacing retained ones.

## Grid Snapshot

The visible prefix of each axis, an Arrangement, and optional focused depth slice. Straight-on and angled orthographic arrangements view the same connected 3D geometry; a flat-looking grid can still contain depth. Reassociated arrangements regroup the tuples. Layout supplies position, extent, and cutaway targets; ordinary Property Tracks supply motion. Extents reveal a connected grid without scaling individual cells; the projection deliberately centers its currently sampled visible bounds during growth. A focused slice clips away other layers without deleting their identities. Reassociation changes `((a, b), c)` into `(a, (b, c))` without adding or losing tuples. Growing a product is not an isomorphism between the smaller and larger sets.

A cell's primary symbol and secondary label are representations of its tuple,
not its identity. Row, column, and depth headings describe the values along the
edges and remain separate from the cell labels.
Their growth-edge disclosure follows the sampled extent along each catalog axis,
so supporting labels appear as the grid reaches them rather than on independent
timers. Semantic visibility (for example regrouping) remains a Property Track.

`GridStylePlan` controls presentation without changing tuple identity: checkerboard,
background-matching, uniform, or row-banded material; full, row-only, or no rules;
and optional table layout. Background-matching material remains opaque to rear
faces and labels. `GridTableLayout` supplies unequal column widths, row height,
padding, alignment, and display headings independent of catalog names. This
one-layer view fixes its placement against the complete catalog rather than
recentering visible prefixes, so headers and retained rows stay still during
growth. Table headings belong to the table plane; the cube's outside headings
remain upright. A conventional keyed-record update/sort model is not yet provided.

## Value Token

A stable actor depicting one value or one explicitly labeled case shape in a
finite teaching diagram. Its immutable label and optional detail describe that
role; equal text in different roles does not imply shared identity. Position,
presence, and border emphasis are ordinary Continuous Channels. A group of Value
Tokens does not imply automatic enumeration or a generic mathematical set API.

## Renderer Recipe

A concrete rendering adapter selected by an actor declaration. Recipe payloads and pixels remain renderer-owned; the Scene Plan core validates identity and timing without understanding their visual implementation.

## Render Window

A positive exact range on the global scene clock selected for delivery. Output begins at time zero, intersecting media is trimmed and rebased, and visual sampling retains global time so ongoing trajectories do not restart.

## Presentation Step

An explicitly ordered destination in a Scene Plan, with a stable ID, title, entry start, and held endpoint on the authored scene clock. Entry start may equal the held endpoint for a still step. Unlike a Cue, a Presentation Step is a navigation destination, and step entry ranges cannot overlap. The native player derives scalar target poses at held endpoints, then animates between those destinations on its own pausable clock. Entry start supplies the explicit Replay pose. Authors choose meaningful endpoint poses; live waiting never alters the automatic video schedule.

## Presentation Deck

An ordered collection of titled Scene Plans. Each slide owns its Presentation Steps and local Playback clock. Changing slides preserves the selected step and pauses the departed slide; returning resumes only motion that was running when it was left. Slide navigation is separate from step navigation.

## Reel

An ordered sequence of independently authored Scene Plans delivered as one video
on a single clock. Each segment keeps its own actors and local time, so a segment's
choreography never depends on its position in the reel. A transition overlaps a
segment with its predecessor: a crossfade mixes the incoming frame over the
outgoing one, while a dip fades the outgoing segment to the empty background before
the incoming one appears, so dense frames never overlap. A zoom flies into a focus
rectangle of the outgoing frame, such as a card, while the incoming segment grows
out of it, so a detail visibly becomes the next scene. A **Wipe** sweeps a divider
across the frame with the incoming segment behind it; its holds rest the divider
mid-frame so a before/after comparison shows both segments side by side, each on
its own running clock, before the sweep goes on. At most two segments are
visible at any instant. Segment media is retimed onto the reel clock for one audio
mix. Unlike a Presentation Deck, a reel is delivered rather than navigated.

## Playback

Interactive navigation among Presentation Steps. Next and Previous retarget continuous channels from their sampled position and velocity through the same Property Track compiler used for video. Unchanged channel destinations retain their trajectories. Pause freezes the local clock without losing motion state; Replay deliberately restarts from an entry pose. Playback retains an immutable compiled timeline for each navigation revision, so late rendered frames cannot change the current destination. It is not reverse playback of a movie.

An explicitly opted-in Start Delay may stagger a property only from a specified
resting pose toward a specified destination. Changing that destination cancels
its unstarted writes; unchanged destinations keep their due times. Once moving,
redirection is immediate and preserves position/velocity. Pausing freezes both
motion and pending starts on the same local clock; reduced motion cancels waits.
Header word entrances demonstrate this contract without callbacks or a second clock.

Playback Speed scales wall-clock elapsed time into local scene time without
changing trajectories, scene-time velocity, or scheduled starts. Diagnostic
frame-stepping explicitly samples that same Timeline backward/forward while
paused, bounded below by the latest navigation time. It is not Previous navigation
and does not reconstruct or reverse earlier destination decisions. Export timing
and sampling FPS remain independent of these native inspection controls.

## Presentation Theme

A named paint palette, independent of Scene Plan identity, typography measurement,
and motion. Original, Evergreen, Tokyo Night, Pure Black, and OpenCode (the OpenCode
TUI's dark tokens) can be selected during
native playback without advancing its clock. The native preference is saved;
file delivery selects a theme explicitly so a personal preference cannot silently
change an export. Original preserves existing scene colors. Semantic status colors
and explicit non-palette art colors are not indiscriminately tinted.

## Rich Text

Immutable Markdown in a bounded overlay, shaped as proportional rich runs with
monospaced code. Paragraphs, headings, list items, and quotations retain their
measured block placement while ordinary actor/block opacity and position channels
animate them. This is not automatic identity matching between edited Markdown
documents. A width-revealing expression instead uses authored stable inline parts.

## Venn Diagram

Two stable, explicitly sized set boundaries with independently sampled position,
radius, and roundness. Hatching represents the intersection of their current
geometry, not a delayed overlay or a relationship guessed from label spelling.
The recipe is a bounded overlay, not a type checker or arbitrary set-layout engine.

## Sequence Diagram

Participants with dashed lifelines and time-ordered rows: messages between
lifelines (or a loop to the same one), notes spanning lifelines, and End marks that
stop a participant. Rows are recipe-local identities revealed by ordinary
Continuous Channels (`row.<id>.reveal`, `.opacity`, `.strike`); a revealed message
travels as a packet before its arrowhead and label land. Rows default to a slot per
row, but several rows may share a slot, so a scene can play the broken behavior and
then replay the fixed behavior in the same places by fading one set out. The recipe
depicts a protocol; it does not simulate one.

## Stage

A 2.5D motion-graphics surface for explainers, rendered on the GPU. Elements sit at
world positions seen through a perspective camera: floating cards with status
lines, a particle orb that spins, breathes, and can shatter, curved light beams (connectors that attach to the side of a card facing the other
end and leave it head-on, and enter an orb radially)
that draw, flow, and snap, packets whose light gathers at a port, travels with a
cooling trail, and lands as a small ring, labels, and rings for timers and
shockwaves. Light is local: a packet or a drawing beam lights only the borders it
nears, and an arrival floods in from its socket. World x/y are canvas
pixels at depth zero, so the default camera is pixel exact; depth gives parallax,
depth of field, and draw order. Bright color blooms; the frame gets highlight
rolloff, vignette, chromatic pulses, and grain. Ambient motion (spin, flow, grain)
is a pure function of time, so any frame renders identically in any order.
An orb pulse is an illumination response, independent of its scale and attached
ports. A card's content can settle after its body; its `content` channel controls
the ink's presence, small vertical offset, and sharpening together.
An orb's **Burst** is a reversible destruction clock: gravitational collapse,
hot combustion, an expanding refractive pressure wave, cooling smoke, and
ballistic embers. Its procedural volume and trajectories need no simulation
history. Wires and arrivals pass beneath the intact orb's occluding shell.

## Caption

Short lines of styled CommitMono text in an explainer's terminal voice. Spans carry
a Tone, so one keyword can take the accent while the rest stays plain. The `typed`
channel reveals characters in order and the accent block caret marks the typing
position; alignment uses each line's full width, so centered text never slides
while it appears. An optional chip draws a rounded surface behind the text.

## Rolling Number

A value such as `rc.112`, `0/8`, or `1,383` whose digits roll in place when it
changes, ported from `@kitlangton/rolling-number`. Each digit place (by numeric
run and place value, independent of separators) is a wheel; a change turns every
changed wheel the way that number moved, while unchanged digits stay still. New
places rise in after their room opens and old ones fade out as every glyph glides
to its new position; separators and literals fade rather than roll. Static prefix
and suffix spans slide with the layout. The values and their times belong to the
recipe; a later change redirects wheels from their current position and velocity.
It is a display of authored values, not a numeric tween.

## Tree

A JSON value shown as a foldable, syntax-tinted outline: a plan a program emits,
a config file, an API payload, or a state shape. Every row's identity is its
JSONPath. Opening a node opens room for its children, so rows below slide down
while rows above stay still; folding reverses it and leaves a summary such as
`{…} 4 keys`. A scalar's later values roll in place in its row. Highlighting a
path lights its row. A tree scrolls through a fixed window when it has more rows.

## Plot

A function chart for explaining motion and metrics: curves over an x and a y
axis, each the Scene Program's own sampled points (for a motion, position with
its exact velocity as the slope). The renderer draws a curve on along its
length, rides a dot along it at the shared playhead, and shows the tangent
there as a velocity arrow; it never evaluates a function. Marks label a
moment, such as a retarget. A Plot depicts computed values; it does not compute
them.

## Lanes

A track view of channels over time: a seconds ruler, one lane per channel with
keyframe diamonds at its event times and a sparkline of its values, cue
brackets above the ruler, and a scrubbing playhead. Keys the playhead has
crossed light and cool by playhead distance, so scrubbing either way samples
deterministically. Built from a Scene Plan, its sparklines are the compiled
Property Tracks the renderer would play. Plot and Lanes share one **Axis**
vocabulary (range, ticks, label, unit).

## Callout

A short label on a crisp leader line pinned to something on screen: a dot and
ring mark the Callout Anchor, the leader draws out from it, and the label rises
in at its end. A **Callout Anchor** is a fixed canvas point, an edge of a
positioned Stage element seen through the camera, an edge of an editor
Semantic Target, or a Sequence Diagram participant header or row. Anchors are resolved by the root recipe at every Temporal
Sample, so the callout follows its target with no lag; moving to another
anchor springs weight channels that blend the two resolved positions. Labels
slide back inside the frame rather than leave it, and the leader follows.

## Anchor

A place an overlay pins to whose position only the renderer knows: a fixed
canvas point, an edge of a positioned Stage element seen through the camera
(including a jolt's roll and punch-in), an edge of an editor Semantic Target
after line motion and the panel's projection, or an edge of a Sequence Diagram
participant's measured header or a row's span (a message's arrow, a note, an
End mark) as the diagram moves. An overlay lists its anchors, each
with an optional pixel offset, and `anchor.<id>` weight channels choose among
them; the first holds the overlay until a weight moves. The renderer resolves
every weighted anchor at every Temporal Sample and the overlay draws at the blended
point, so it never lags its target. Moving between anchors springs the weights,
so an interrupted move keeps its velocity. Callouts, captions, Rolling Numbers,
text, and images share this model. An anchor names a target; it does not parent
one actor to another, so there is no hierarchy, and a pinned overlay follows its
target's position, not its scale.

## Image

One planned image media placement (PNG, JPEG, or WebP) drawn bare or inside a
framed card, through the same projected card as a Video Card, so it can move,
scale, rotate, tilt, defocus, and pin to an Anchor. The file is decoded once;
its width at rest is authored and its height follows the image.

## Tone

A semantic color role shared by explainer recipes: plain, request, success,
error, warning, muted, and accent. Status tones keep one color in every
Presentation Theme; the others follow the palette.

## Line Mark

A diff decoration on one editor line: added or removed. The mark tints the row,
draws an accent bar and a vector +/- sign in the gutter, and joins consecutive
marked rows into one band. Its presence is the `mark.<line-id>` Continuous Channel,
so a removed line can turn red just before a Code Snapshot removes it.

## Stepped Diff

One code change told as ordered steps over Stable Lines: each line is kept,
added in a step, or removed in a step. Added lines carry an added Line Mark;
removed lines turn red just before their step. A pure insertion or removal first
holds blank rows so moving code never crosses entering or leaving code.

## Asset

Immutable source material identified independently from any use on the timeline. Audio, video, and image assets retain their original files while edits refer to them non-destructively.

## Clip

One positive-duration source range from an audio or video asset. Moving, copying, removing, or changing an audio clip's gain changes the edit without changing its source asset.

## Script Clip

A clip on the primary spoken-media track. Its transcript may drive structural edits, captions, and semantic timing.

## Layer Clip

Accompanying timed media such as music, sound effects, or B-roll. Layer clips share the media clock but do not implicitly become part of the editable transcript.

## Cue

A named timeline range. Cues may be authored or imported from transcript word and phrase timing; their start and end can synchronize motion and media.

## Transcript

An ordered set of words with source start and end times. Looking up a word occurrence produces a cue range on the same exact media clock used by script clips. A phrase lookup matches consecutive whole words, ignoring case and punctuation and treating number words as digits, so choreography can be keyed to what narration says rather than to seconds; re-voicing a clip re-times the scene.

## Media Placement

A compiled relationship between a clip's immutable source range and its scheduled timeline range. Media placement time uses integer nanoseconds so edit boundaries remain exact across repeated edits.

## Property Track

The compiled trajectory of one scalar actor property. A later spring on the same track begins from the earlier trajectory's sampled position and velocity.

A spring's settling time is deterministic. After that time the segment stays exactly at rest unless another segment retargets it; starting an unrelated channel cannot wake it.

## Semantic Target

A stable named selection owned by an actor recipe, such as a logical range inside a stable code line. The plan core preserves identity and validates references; the renderer recipe measures concrete geometry. Highlights and pointers attach to Semantic Targets rather than authored screen coordinates.

Attachment follows sampled visible layout, including collapsed Inline Reveals and moving Stable Lines. Expanded text measurements are a preparation baseline, not final coordinates for every step.

## Inline Reveal

A transition that expands or collapses authored spans inside a Stable Line. Variable spans animate width, opacity, and blur while common prefix, infix, and suffix spans retain identity and move to their new positions without replacement. Opposing reveals can exchange slot alternatives horizontally while preserving maximum stability.

## Pointer

A stable visual actor that directs attention to a semantic target. Its position and opacity are ordinary property tracks, so retargeting and temporal sampling use the same motion system as every other actor.

## Task

A stable visual actor representing one Effect computation. The `effect-task` recipe schedules its idle, running, succeeded, failed, death, hidden, and retry state changes; the stable task ID preserves identity across those changes.

## Task State

One meaningful snapshot of a Task. A task state selects semantic content and visual targets, while the renderer derives the transition from the preceding state at arbitrary media time. A succeeded task may carry a result or represent payload-free completion.

Planned interactive Tasks lower state changes into continuous geometry and content-presence tracks. A running Task can keep the local playback clock active after those tracks settle, while pause and reduced motion still stop its ambient animation. It illustrates a computation rather than executing an actual Effect.

## Video Card

One planned video media placement played inside a framed card: rounded corners,
a theme-aware material, border, and shadow, and an optional title bar. The plan
clock maps through the placement into source time, so a cue or range render shows
the same footage frame as the full film. Its focus window crops into a region of
the frame (a zoom into part of a screen recording) while the card itself can move,
scale, and tilt. Recordings remain evidence of actual product behavior rather than
a reconstructed simulation; explanatory text is ordinary overlay actors.

## Temporal Sample

One evaluation of the complete scene within an output frame's shutter interval. Psychopomp combines a frame's weighted temporal samples (its exposure) to produce motion blur from real scene movement; a Stage adds their light on the GPU before developing the frame.

## Editor Frame

The renderer-neutral description of one sampled editor scene: panel position, focus state, and placed code lines.
