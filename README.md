# Kinograph

Code-first motion graphics in Rust. One authored scene can become a native,
interruptible presentation or a shutter-sampled video.

Kinograph is an early prototype, not a general-purpose scene graph. Its examples
explore stable code edits, teaching diagrams, typography, and simulated interfaces.
Motion is sampled at arbitrary times; reversing a transition preserves its
current position and velocity instead of restarting an animation.

## Run a presentation

From the repository root:

```sh
cargo run -p kinograph-opencode-architecture
cargo run --release -- plan present target/opencode-architecture/deck.json --theme original
```

This opens the four-step Daemon diagram. **1 / 2** selects Flat / Isometric;
**← / →** changes steps; **R** replays; **P** pauses; **S** slows motion.

You need a recent Rust toolchain, a working `wgpu` adapter, and a desktop display.
The prototype has been exercised on macOS/Metal. CommitMono is preferred; the
renderer falls back to system monospace when its configured font is unavailable.

For the typography, table, and component showroom, see
[Scene Programs and presentations](SCENE_PLANS.md).

## Export the same scene

With FFmpeg and `libx264` on `PATH`:

```sh
cargo run --release -- plan render target/opencode-architecture/daemon-isometric.json output/daemon.mp4 --range 3..4.2 --theme original
```

The range samples the original scene clock, so cutting into a transition does
not restart it. Exports choose their theme explicitly; native preferences do not
silently change exported pixels. Generated plans and media belong in ignored
`target/` and `output/` directories.

## Make a narrated explainer

`scenes/pr-walkthrough` is a complete narrated reel: sequence diagrams replay broken
and fixed behavior, and editors animate each change as a diff. See
[Make A Narrated Explainer Reel](SCENE_PLANS.md#make-a-narrated-explainer-reel).

```sh
cargo run -p kinograph-pr-walkthrough
cargo run --release -- plan render scenes/pr-walkthrough/pr-walkthrough.reel.json output/pr-walkthrough.mp4 --theme opencode
```

## Change intent, motion, or pixels in the right place

A **Scene Program** is a small Rust executable under `scenes/`. It writes a
**Scene Plan**: JSON containing identities, destinations, timing, and recipe data.
The renderer prepares that plan once, then samples it for either delivery.

```text
scenes/*                         authored meaning, destinations, choreography
    ↓ Scene Plan
crates/kinograph                  identity, validation, tracks, retargeting, time
    ↓ typed preflight and resource preparation
crates/kinograph-render           measured typography, recipes, sampled pixels
    ├─ native presentation       Playback, window and worker scheduling
    └─ video export              temporal sampling, readback and FFmpeg
```

Share a rule when two real callers need the same behavior. Keep recipe-specific
layout, identity, and motion choices visible. A Grid product, an editor line,
and a deployment row are not interchangeable just because each has a key.

The [browser experiment](experiments/browser-grid-prototype/README.md) reuses
selected Grid/Diagram code through WASM/WebGPU. It remains an isolated prototype
with baked labels—not a supported browser renderer or editable-text API.

## Verify a change

```sh
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

For rendering or choreography changes, also inspect targeted frames and
transitions, including interrupted navigation. Passing tests is not aesthetic
approval. The browser experiment has separate build and proof commands.

## Read further by question

| Question | Document |
| --- | --- |
| What do the domain terms mean? | [CONTEXT.md](CONTEXT.md) |
| Which Module owns this behavior? | [ARCHITECTURE.md](ARCHITECTURE.md) |
| How do I author, inspect, present, or export? | [SCENE_PLANS.md](SCENE_PLANS.md) |
| What is in scope, and what comes next? | [PLAN.md](PLAN.md) |
| Which references inform the motion? | [PRIOR_ART.md](PRIOR_ART.md) |
| Where do composable particle and shader effects live? | [EFFECTS.md](EFFECTS.md) |
| What did earlier experiments establish? | [NOTES.md](NOTES.md) and [perf/](perf/) |

[AGENTS.md](AGENTS.md) records the engineering, stability, and verification rules.
Current contracts live in the domain and architecture documents; experiment
history records evidence and superseded trials, not a second specification.
