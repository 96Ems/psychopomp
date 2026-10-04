# Psychopomp

Code-first motion graphics in Rust. A small Rust program describes a scene;
Psychopomp renders it as a 1080p60 video with real motion blur, or plays it as
an interactive presentation.

Every frame is a pure function of time, so any frame renders identically in any
order, and interrupted motion keeps its velocity. It is an early, thoroughly
vibe-coded prototype, tested on macOS (Metal).

## What it draws

- **Stage**: a 2.5D camera over particle orbs, cards, wires, travelling packets,
  labels, and rings, with bloom, depth of field, screen shake, zoom streaks,
  explosions, and a VHS rewind. The camera frames, follows packets, racks focus,
  orbits, dolly-zooms, whips, and sways handheld.
- **Code**: an editor that animates diffs while every line keeps its identity.
- **Overlays**: callouts pinned to anything, rolling numbers, captions, sequence
  diagrams, charts, trees, and video cards.
- **Narration**: optional ElevenLabs or Fish Audio voice-over. Each beat waits
  for the word that triggers it, so re-voicing re-times the film.

## Example

```rust
let plan: StagePlan = serde_json::from_value(serde_json::json!({
    "elements": [
        { "kind": "card", "id": "client", "at": [560, 540, 0], "size": [300, 110], "title": "client" },
        { "kind": "orb", "id": "server", "at": [1360, 540, 0], "radius": 140 },
        { "kind": "beam", "id": "link", "from": "client", "to": "server" },
        { "kind": "packet", "id": "hello", "beam": "link", "label": "GET /hello" }
    ]
}))?;
let mut scene = PlanBuilder::new("hello", 4 * SECOND);
let mut stage = StageActor::declare(&mut scene, "stage", &plan)?;
let ready = stage.settle_in(&mut scene, "client", 0); // the card drifts into place
let wired = stage.connect(&mut scene, "link", ready, 0.6); // the wire draws on
let landed = stage.send(&mut scene, "hello", wired + SECOND / 2, 0.8); // a packet flies
stage.land(&mut scene, "server", landed); // the orb lights up
stage.jolt(&mut scene, landed, [1.0, 0.0], 0.6); // and the camera takes the hit
std::fs::write("target/hello.json", serde_json::to_string_pretty(&scene.finish()?)?)?;
```

The full program is [`scenes/hello`](scenes/hello/src/main.rs). Run it and render
the plan it writes (you need a recent Rust toolchain, a GPU, and FFmpeg with
`libx264`):

```sh
cargo run -p psychopomp-hello
cargo run --release -- plan render target/hello.json output/hello.mp4 --theme neutral
```

Check frames without encoding a video:

```sh
bun scripts/sheet.ts target/hello.json 0.5,1.5,2.7 --theme neutral --shutter
```

## Examples

| Scene | What it shows |
| --- | --- |
| [`psychopomp-intro`](scenes/psychopomp-intro) | This library introducing itself, loudly |
| [`2password`](scenes/2password) | A narrated product explainer on the Stage |
| [`pr-walkthrough`](scenes/pr-walkthrough) | Pull requests as Stage films that zoom into their diffs |
| [`camera`](scenes/camera) | A Stage diagram shot like a film: every camera move |
| [`callouts`](scenes/callouts), [`rolling-number`](scenes/rolling-number), [`charts`](scenes/charts), [`tree`](scenes/tree), [`diagnostics`](scenes/diagnostics) | Component showrooms |
| [`effects-showroom`](scenes/effects-showroom) | Lightning, charge, shields, dissolve, and scans on the Stage |
| [`interactive-showcase`](scenes/interactive-showcase) | A native, steppable presentation (`plan present`) |

## Use it with a coding agent

The repo ships two skills in [`.agents/skills`](.agents/skills): `psychopomp` (the
reel workflow: facts, script, narration, choreography, review, render) and
`explainer-motion` (how to make diagrams move like physical things). Agents that
read `.agents/skills` pick them up inside this repository. To install them
elsewhere:

```sh
npx skills add kitlangton/psychopomp
```

## How it fits together

```text
scenes/*              Rust Scene Programs: meaning, timing, choreography
   ↓ Scene Plan (JSON)
crates/psychopomp     plans, validation, timelines, springs; no GPU
   ↓
crates/psychopomp-render   wgpu rendering → native presentation or FFmpeg video
```

| Question | Read |
| --- | --- |
| What do the terms mean? | [CONTEXT.md](CONTEXT.md) |
| Where does a behavior live? | [ARCHITECTURE.md](ARCHITECTURE.md) |
| How do I author, present, or render? | [SCENE_PLANS.md](SCENE_PLANS.md) |
| Which effects exist, and how are they built? | [EFFECTS.md](EFFECTS.md) |
| What inspired the motion? | [PRIOR_ART.md](PRIOR_ART.md) |

## Develop

```sh
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

[AGENTS.md](AGENTS.md) has the engineering and verification rules. CommitMono is
bundled under the SIL Open Font License (`assets/fonts`).
