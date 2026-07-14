# Kinograph Agent Guide

Kinograph is an early Rust prototype for deterministic, code-first motion graphics. The current benchmark is one headless 1920x1080 code-animation scene rendered with `wgpu`, shaped with `cosmic-text`, temporally sampled for motion blur, and streamed to FFmpeg.

## Read First

- `CONTEXT.md` defines the domain language. Use its terms in code and documentation.
- `ARCHITECTURE.md` describes the current module boundaries and intentional non-abstractions.
- `PLAN.md` defines the product direction, milestones, scope cuts, and success criteria.
- `NOTES.md` records what the prototype has and has not proved.
- `PRIOR_ART.md` records the animation systems that should inform timeline and authoring API work.

Keep these documents accurate when a change alters a domain term, architectural boundary, validated finding, or milestone direction. Do not duplicate their detail here.

## Commands

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo run --release
```

`cargo run --release` renders `output/kinograph-prototype.mp4` by default. Pass an output path as the first argument to override it. A full render requires:

- a working headless `wgpu` adapter
- `ffmpeg` with `libx264` on `PATH`
- CommitMono at the path currently declared in `src/main.rs`; rendering falls back to the system monospace font if it is unavailable

Do not run the full render as routine validation when unit tests and static checks cover the change. Run it when changing rendering, typography, choreography, temporal sampling, readback, or encoding behavior.

## Architecture

- `src/code.rs`: stable line identity, code documents and snapshots, validation, and sampled line placement
- `src/dsl.rs`: public Rust scene values, semantic targets, actor helpers, and lowering into scalar tracks
- `src/motion.rs`: deterministic arbitrary-time analytic spring sampling with position and velocity
- `src/render.rs`: concrete headless `wgpu` renderer and `cosmic-text` sprite compositor
- `src/encode.rs`: concrete FFmpeg subprocess and raw RGBA frame protocol
- `src/main.rs`: visible prototype choreography and application wiring
- `src/lib.rs`: public library module boundary used by the Rust authoring DSL
- `src/scene.wgsl`: editor geometry and focus shader

Preserve these boundaries unless a concrete scene or second implementation demonstrates a better seam. In particular, do not introduce a generic scene graph, renderer or encoder traits, plugins, extra crates, or a speculative authoring DSL merely for future flexibility.

## Engineering Rules

- Prefer the smallest change that improves the benchmark scene or answers a stated prototype question.
- Keep scene choreography visible in `main.rs` until repeated authoring operations justify compilation behind a deeper interface.
- Keep code identity and layout independent of glyphs, GPUs, colors, FFmpeg, and absolute screen coordinates.
- Keep layout responsible for target placement and motion responsible for trajectories.
- Preserve deterministic arbitrary-time sampling. Do not replace trajectories with stateful frame-by-frame integration.
- Carry both position and velocity when introducing interrupted or redirected motion.
- Add abstractions only after a real second use or implementation exposes the seam.
- Avoid unsafe code and codec bindings unless measured evidence shows the subprocess boundary is insufficient.
- Treat output media and build artifacts as generated files; keep them under ignored `output/` and `target/` directories.

## Testing

- Add focused unit tests beside pure domain logic in `code.rs` and `motion.rs`.
- Test observable invariants such as stable identity, endpoint placement, velocity continuity, validation failures, and deterministic out-of-order sampling.
- For renderer or encoder changes, supplement automated checks with a targeted artifact render and inspect the result at full scale.
- When performance changes, compare the same resolution, frame count, temporal sample count, and build profile before claiming an improvement.

Before finishing a code change, run `cargo test`, `cargo fmt --check`, and strict Clippy. State explicitly if GPU, font, or FFmpeg constraints prevented artifact-level verification.
