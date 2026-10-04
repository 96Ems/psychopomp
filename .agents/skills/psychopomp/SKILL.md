---
name: psychopomp
description: >
  Make explainer reels and deterministic motion graphics with Psychopomp:
  walkthrough videos of pull requests, code changes, protocols, or lifecycles,
  with GPU Stage choreography, callouts, rolling numbers, sequence diagrams,
  animated diffs, and optional ElevenLabs or Fish Audio narration.
---

# Psychopomp explainer reels

An **explainer reel** is several independently authored Scene Plans joined on one
clock: behavior stories on the GPU Stage (orbs, cards, beams, packets, camera,
bloom), pinned Callouts, Rolling Numbers, changes as animated Stepped Diffs,
captions in a terminal voice, and optional spoken narration. When narrated,
every visual beat waits for a phrase in the transcript, so re-voicing re-times
the film automatically.

The working examples are in `scenes/pr-walkthrough`: `src/flagship.rs` (#50825 as a
Stage film that zooms into its code), `src/stop_stage.rs` (#50042 combining Stage,
Callouts, Rolling Number, and a Zoom into an annotated code diff), and `src/lib.rs`
(the five-PR reel with sequence diagrams). Copy their shape; do not start from a
blank crate. Component payloads, channels, and commands live in `SCENE_PLANS.md`
("Make A Narrated Explainer Reel") and terms in `CONTEXT.md`; read both before
authoring. Engineering rules are in `AGENTS.md`.

## Steps

1. **Establish the facts.** For each change, read the actual diff and PR description
   (`gh pr diff`, `gh pr view`) and write one sentence each for the broken behavior,
   the fixed behavior, and the change. Done when every claim you will narrate is
   traceable to code, a test, or a reproduced run; mark anything you could not verify
   and keep it out of the script.

2. **Write the script** as `scenes/<name>/narration/script.json`, one clip per part:
   intro, then per change `<slug>-before`, `<slug>-after`, `<slug>-code`, then outro.
   Follow [STORY.md](STORY.md) when writing it. Done when each clip is under about 30
   seconds spoken and every planned visual beat has a distinct anchor phrase in it.

3. **Voice it (optional).** Draft first with no credentials (`--draft` uses macOS
   `say`), or synthesize final narration with **ElevenLabs** (`engine: "elevenlabs"`,
   default model `eleven_v4`) or **Fish Audio** (`engine: "fish"`):
   ```json
   {
     "engine": "elevenlabs",
     "model": "eleven_v4",
     "voice": "<elevenlabs-voice-id>",
     "settings": { "stability": 0.5, "similarity": 0.7 },
     "clips": [
       { "id": "change-before", "text": "[warm, conversational voice] ..." }
     ]
   }
   ```
   ```sh
   # Draft (no API key required)
   bun scripts/narrate.ts scenes/<name>/narration/script.json --draft

   # ElevenLabs (requires ELEVENLABS_API_KEY and script.voice)
   ELEVENLABS_API_KEY=... bun scripts/narrate.ts scenes/<name>/narration/script.json

   # Fish Audio (requires FISH_AUDIO_API_KEY)
   FISH_AUDIO_API_KEY=... bun scripts/narrate.ts scenes/<name>/narration/script.json
   ```
   `scripts/narrate.ts` normalizes loudness, transcribes word timings with Whisper,
   and writes `<id>.mp3`, `<id>.words.json`, and `narration.json`. When switching
   voices or engines, rebuild the Scene Plan against the new word timings.

4. **Author the Scene Program** in `scenes/<name>` (add it to the workspace). Behavior
   stories are Stage films built with `StageActor` (see `flagship.rs` and
   `stop_stage.rs`). Schedule the clips with `Narration::reading`, build elements
   with `StageElement::card|orb|beam|packet|label|ring` on `StagePost::RESTRAINED`,
   frame the film with `psychopomp::chrome` (header, chips, footer), and play
   sounds from `psychopomp::sfx`. Load the `explainer-motion` skill and apply it
   to every beat: cards `settle_in`, beams `connect`, messages `send`, impacts
   `hit`, the hero `orb_in`, the fix's `rewind`; use
   `CalloutActor` to pin annotations to Stage elements or Editor code ranges and
   `RollingNumberActor` for live counters/timers; then camera moves, rewind,
   resolution, and a `zoom` into the code. Reach for the visualization overlays
   instead of hand-building them from Stage labels and rings: `ChecklistActor`
   for checks that run and resolve, `MeterActor::countdown` for a timeout ring,
   `BarsActor` for before/after numbers, `ConfettiActor` for a success beat, and
   `SubtitlesPlan::from_spoken` to burn in word-timed captions
   (`scenes/viz-components` shows all five).
   Changes are `Diff`s of `keep`, `add(step)`, and `remove(step)` lines, each step
   keyed to a phrase. Done when `cargo run -p <crate>` writes the reel and
   `cargo run --release -- plan validate <reel>` reports valid. A missing phrase panics
   with its clip: change the anchor to words the transcript actually contains.

5. **Review before rendering.** Get segment spans from `plan inspect <reel>`, then
   contact-sheet each segment at its before, switch, after, and code beats:
   ```sh
   bun scripts/sheet.ts <reel> t1,t2,... --theme neutral --shutter --out output/sheet.jpg
   ```
   When tuning the look, set `PSYCHOPOMP_SHADER_DIR=crates/psychopomp-render/src/render`
   and edit `stage.wgsl`/`stage_post.wgsl`: every frame and sheet picks up shader edits
   without a Rust rebuild.
   Then render one behavior segment with audio (`plan render <reel> out.mp4 --cue <scene-id>`)
   and inspect frames extracted during motion. Done when every segment has been
   looked at and no text overlaps, clips, or reads against the wrong chip.

6. **Render and verify** the whole reel:
   ```sh
   cargo run --release -- plan render <reel> output/<name>.mp4 --theme neutral
   ```
   Done when `ffprobe` shows 1920x1080, 60 fps, AAC audio, and a duration matching
   `plan inspect`, and loudness is near -16 LUFS with peaks under -1 dBFS.

7. **Deliver** the MP4 path. Commit the scene, narration assets, and any engine
   changes in Psychopomp with conventional messages.

## Improving Psychopomp

When a reel needs something the engine cannot do, add it to Psychopomp rather than
working around it in one scene: plan types and validation in `crates/psychopomp`,
strict-channel preflight in `plan_runtime`, pixels in `render`, GPU-free tests plus
an `#[ignore]` GPU test, and the docs `AGENTS.md` asks you to keep current.

Build from small reusable pieces. Interpolation, easing, curves, shape ports, and
connectors belong in `psychopomp::math` (organized like pmndrs `math`: core `lerp`/
`remap`/`smoothstep`, `easing`, `curve`, `shapes`, `random`; glam vectors). Extend it
instead of inlining math in a renderer or scene, and split renderers into one small
helper per element, as `render/stage.rs` does. Keep
`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
and `cargo test --workspace` green.
