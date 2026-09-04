# Scene Programs Compile Into Inspectable Plans

Kinograph keeps Rust as its authoring language. A Scene Program can import libraries, load data, run calculations, use loops, and define helpers. Running that program emits a versioned Scene Plan containing only stable actors, continuous channels, state channels, cues, and media placements.

```text
Rust Scene Program -> Scene Plan -> persistent kinograph-render process
```

The separation keeps scene compilation lightweight and lets one renderer process retain its GPU device, font caches, and rendering implementation across repeated agent requests.

## Run The Example

Emit a plan from the lightweight example Scene Program:

```bash
cargo run -p agent-demo -- target/agent-demo.json
```

Emit the canonical editor-heavy hero plan:

```bash
cargo run -p kinograph-hero -- target/hero.json
```

Emit the narration-rich OpenCode session-tool lesson:

```bash
cargo run -p kinograph-opencode-session-tool -- \
  scenes/opencode-session-tool/opencode-session-tool.plan.json
```

Emit the state-driven deployment queue UI proof:

```bash
cargo run -p kinograph-deployment-queue -- \
  scenes/deployment-queue/deployment-queue.plan.json
```

Emit the minimal Solid Store to Quark before-and-after:

```bash
cargo run -p kinograph-quark-before-after -- \
  scenes/quark-before-after/quark-before-after.plan.json
```

Inspect or validate the result without initializing a GPU:

```bash
cargo run -- plan schema
cargo run -- plan validate target/agent-demo.json
cargo run -- plan inspect target/agent-demo.json
```

Compare two validated plans or render one exact PNG frame:

```bash
cargo run -- plan diff target/before.json target/after.json
cargo run --release -- plan frame target/agent-demo.json 1.25 output/frame.png
```

Render only one cue or exact range on the original scene clock:

```bash
cargo run --release -- plan render target/agent-demo.json output/intro.mp4 --cue intro
cargo run --release -- plan render target/agent-demo.json output/window.mp4 --range 0.2..0.4
```

A Render Window trims and rebases intersecting media to output time zero, but visual sampling remains on the original global scene clock. Starting a window in the middle of a spring therefore preserves its position and velocity.

Delivery remains frame-based: a window whose duration is not exactly frame-aligned emits one final frame sampled only within the remaining window interval. At 60 fps, the encoded duration therefore rounds up to the next frame boundary.

## Play As A Presentation

Scene Plan v2 has optional `presentationSteps` metadata. Omitting it preserves
existing serialized plans and video behavior. Steps are separate from cues: each
has a stable ID, title, entry start, and exact held endpoint. They are ordered and
non-overlapping, and may be still-only (`startNanos == holdNanos`). Authors must
choose meaningful hold times; validation checks timing, not visual settling.

```rust
scene.presentation_step("initial", "Start with a value", 0, 0);
scene.presentation_step("reveal", "Reveal the type", 1_000_000_000, 2_500_000_000);
```

Build the Effect Institute `effect-succeed` adaptation:

```bash
cargo run -p kinograph-effect-succeed-slides -- target/effect-succeed-slides.json
cargo run --release -- plan present target/effect-succeed-slides.json
```

This opens a native Rust window and samples the prepared scene directly, without
exporting clips. Left/Right animate toward the previous/next held pose; another
keypress immediately redirects the current springs without dropping velocity.
R explicitly restarts from the current step's entry pose. Space/P pause or resume,
Home/End select first/last, F toggles full screen, and Escape closes the window.

X toggles smooth/pixelated scaling; M toggles reduced motion. `--reduced-motion`
starts without transition motion, and `--full-quality` disables the fast flat-editor
preview profile. The default profile caches unchanged chrome and omits final
optical resampling of code glyphs, while preserving the same motion tracks. Resizing
scales the authored canvas rather than reflowing it.

Fractional glyph sampling, reveal-edge coverage, and continuous blur happen before
window scaling. Dynamic focus/highlight overlays reuse the WGSL recipe without
rebuilding static chrome through the optical compositor. Semantic coordinates
follow sampled visible widths and line positions; literal coordinates remain literal.

Final window scaling is GPU-backed with an sRGB texture and linear/nearest
sampling. `--benchmark` runs a ten-second native interruption sequence and prints
frame-submission timing; it keeps its window on top and exits automatically.

Native pacing follows the current monitor's reported refresh rate (including
120/144 Hz), falling back to 60 Hz when unavailable. `--fps 120` overrides the
sampling cap without changing video FPS or disabling FIFO synchronization. A
60 Hz display still cannot show 120 distinct frames/sec. `--benchmark-gpu` adds
explicit GPU-completion waits to the benchmark and separates completed rendering
work from waiting for a drawable; it is a diagnostic mode, not normal playback.

The interruptible player accepts editor, pointer, text, title-card, and planned
Effect Task scenes. Task recipe state changes lower into continuous visual tracks;
generic State Channels and recorded-media scenes are still rejected until
their interactive timing is defined. The same Scene Plan still exports as MP4
through `plan render`, with its original timing and media placements. Live source
reloading, native higher-DPI glyph rasterization, and presentation audio remain open.

### Present A Deck

```bash
cargo run -p kinograph-interactive-showcase
cargo run --release -- plan present target/interactive-showcase/deck.json
```

The four-slide demo includes the original code reveal, Effect Task lifecycle/retry,
parallel Tasks, and keyed code insertion/removal with an attached highlight.
`DeckPlan` v1 contains an ID and titled `SlidePlan` values, each embedding an
independent Scene Plan. Single-plan presentation remains supported.

- **`'` / Shift+`'`:** next/previous slide, wrapping around.
- **1–9:** jump directly to a slide.
- **Left/Right:** previous/next step within the active slide.
- **Home/End:** first/last step within that slide.
- **Space/P, R, M, X, F, Escape:** pause, replay, reduced motion, filtering,
  fullscreen, and close, as in single-plan presentation.

Inactive slides retain their step and freeze their local clock. Returning resumes
only motion that was running when the slide was left; explicit pause stays paused.
Running Tasks keep animating after their dimensions settle, until paused or advanced.
Reduced motion freezes their ambient animation too. The showcase program emits
individual slide JSON files beside the deck for `plan frame`, `plan render`, and
`plan steps`; export those Scene Plans, not the deck wrapper.

For a deck, `--benchmark` also switches slides every two measured seconds. Those
results include slide-switch costs and are not directly comparable with a
single-scene throughput benchmark.

### Mask Rolling Text

The `text` recipe accepts an optional canvas-space `verticalMask`:

```json
{"text":"Run the computation","center":[960,780],"fontSize":30,
 "verticalMask":{"top":750,"bottom":810,"fade":12}}
```

The mask stays fixed while the actor's `y` channel moves the text through it.
Coverage ramps linearly from zero at `top` to full opacity at `top + fade`, stays
full in the middle, and falls to zero at `bottom`. Everything outside is clipped.
`fade` may be zero for a hard aperture, but cannot exceed half its height.
`plan validate` checks finite ordered bounds and the fade range without a GPU.

This is a text alpha mask, not a dark rectangle composited over the scene.
Other actors and the background remain unchanged. The showcase captions use a
shared 60-pixel aperture and 12-pixel edge fades, following `visual-types`' rolling
content treatment. Existing opacity channels still prevent skipped, unselected
captions from appearing while their y destinations change.

### Inspect Maximum Stability

```bash
cargo run -- plan steps target/effect-succeed-slides.json
```

No GPU is initialized. Each editor step reports before/after text, `beforeDelta`,
`delta`, changed part IDs, retained-line movement, and before/after y positions.
`«…»` marks changed parts, not unchanged text displaced by neighboring layout.
Warnings identify common text inside exchanged ranges or replaced lines, and
held endpoints that still contain moving or partially visible code. Warnings are
heuristics: they do not merge semantically different IDs. For partial holds,
reported text describes participating parts, not the exact clipped glyphs.
The persistent server accepts `{"id":1,"command":"steps","plan":"path.json"}`.

### Schedule Several Line-Order Changes

Leave `EditorRecipePlan.snapshots` empty for the original shared `layout`/`content`
placement. For keyed edits, declare every possible line once, retain the opening
`initial_line_ids`, and schedule later orders:

```rust
recipe.snapshots = vec![
    EditorSnapshotPlan {
        at_nanos: 1_000_000_000,
        line_ids: vec!["import".into(), "helper".into(), "definition".into()],
    },
    EditorSnapshotPlan {
        at_nanos: 3_000_000_000,
        line_ids: vec!["import".into(), "definition".into()],
    },
];
```

The last order must equal `final_line_ids`. A line may be absent from both initial
and final orders but present between them. Preparation derives ordinary per-line
y/opacity tracks with the 0.45-second zero-bounce profile. Equal-time snapshots
coalesce before computing layout; removed lines fade in place. Generated tracks
replace legacy shared placement in this mode; Inline Reveals remain independent.
Use presentation holds after settling (two seconds after a change is ample for
these examples). `plan validate` checks recipe ranges, snapshot timing, and generated
channel collisions without a GPU.

Old JSON plans need no new fields. Rust recipe literals use `snapshots: Vec::new()`
to retain the old behavior. Do not author in the generated `line.<id>.y`,
`line.<id>.opacity`, or `__attachment-*` namespaces.

## Keep The Renderer Running

`kinograph plan serve` reads one JSON request per line from standard input and writes one JSON response per line to standard output. Progress and GPU diagnostics use standard error, leaving standard output machine-readable.

```bash
cargo run --release -- plan serve
```

Inspect a plan:

```json
{"id":1,"command":"inspect","plan":"target/agent-demo.json"}
```

Render one frame while retaining the initialized renderer:

```json
{"id":2,"command":"frame","plan":"target/agent-demo.json","output":"output/frame.png","at_nanos":1250000000}
```

Render an exact range:

```json
{"id":3,"command":"render","plan":"target/agent-demo.json","output":"output/window.mp4","start_nanos":200000000,"end_nanos":400000000}
```

Render a cue:

```json
{"id":4,"command":"render","plan":"target/agent-demo.json","output":"output/intro.mp4","cue":"intro"}
```

Stop the process:

```json
{"id":5,"command":"shutdown"}
```

## Author With Typed Handles

`PlanBuilder` creates stable handles and derives channel IDs from actor identity:

```rust
let mut scene = PlanBuilder::new("lesson", 3_000_000_000);
let title = scene.actor(
    "title",
    "title-card",
    serde_json::json!({ "title": "Effect" }),
)?;
let opacity = scene.continuous(&title, "opacity", 0.0);
scene.spring(&opacity, 200_000_000, 1.0, 0.4, 0.0);
scene.cue("intro", 0, 2_000_000_000);
let plan = scene.finish()?;
```

Renderer Recipe payloads remain adapter-owned. The lightweight core validates stable IDs, channel references, event ordering, finite values, cue ranges, exact media ranges, and Scene Plan versioning without knowing what a Task, editor, terminal, or title card looks like.

Scene Plan v2 scalar values may reference a component of a stable Semantic Target. The target's selector remains recipe-owned; for the hero, the editor recipe resolves logical code range IDs through `cosmic-text` before compiling highlight and pointer channels into the shared Timeline.

The current plan runtime demonstrates `title-card`, `text`, `editor`, attached `pointer`, `effect-task`, `terminal-recording`, and `deployment-queue` renderer recipes. Planned audio lowers into exact script or layer placements for FFmpeg. Planned video is accepted only when a prepared visual recipe consumes its media ID; unconsumed video and all image media still return request errors. The terminal recipe maps the global scene clock through the media placement into source time, so cue and range renders do not restart footage. Editor, terminal, and deployment recipes independently produce RGBA content but delegate framing to the same private immediate-mode card compositor; this reuse does not add recursive presentation nodes to Scene Plan. The deployment recipe compiles ordered semantic snapshots into private stable keyed row tracks, keeping layout destinations distinct from velocity-preserving motion.

## Package Direction

```text
scenes/* ------------> kinograph
                           ^
                           |
kinograph-render ----------+
```

- `crates/kinograph`: lightweight plans, authoring values, motion, composition, stable code, and validation
- `crates/kinograph-render`: concrete renderer, encoder, development server, CLI, and built-in scenes
- `scenes/*`: lightweight Rust Scene Programs

The default hero command embeds `scenes/hero/hero.plan.json` for compatibility. A workspace test regenerates the plan from `scenes/hero/src/lib.rs` and requires byte equality, so the checked artifact cannot drift from its Rust source.

`scenes/opencode-session-tool/opencode-session-tool.plan.json` is likewise checked against its Rust Scene Program. It demonstrates one planned split Vim/OpenCode Terminal Recording, layered SFX, discrete state, continuous panel motion, nine live-capability text overlays, and named cue selection through the same renderer process.

`scenes/deployment-queue/deployment-queue.plan.json` is checked the same way. It demonstrates a typed state-driven UI Surface whose rows retain recipe-local identity across insertion, phase replacement, failure focus, and retry while the Scene Plan remains ordinary actors, continuous channels, state channels, and cues.

The plan and authoring Modules intentionally share one lightweight crate. They should become separate crates only after another language, protocol consumer, or independent version lifecycle demonstrates that seam.
