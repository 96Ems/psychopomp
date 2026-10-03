# OpenCode quality loops — team proposal

A 17-slide **native Psychopomp presentation** and a phrase-timed narrated video,
with progressive reveals and three quiet Stage card-and-beam diagrams. It argues
for a bounded pilot, not a new platform or a claim that the proposed OpenCode
repairs are already complete.

## Present

```sh
cargo run -p psychopomp-opencode-quality
cargo run --release -- plan present target/opencode-quality/deck.json --theme neutral
```

- **⌘→ / ⌘←**: next / previous slide (`'` / Shift+`'` are aliases).
- **→ / ←**: reveal / retreat within a slide.
- **Home / End**: first / last reveal on the current slide.
- **R**: replay; **S**: slow; **M**: reduced motion; **F**: fullscreen.
- **Escape**: close.

Allow about 12–15 minutes plus discussion. Slides 16–17 are appendices.
[`PRESENTER.md`](PRESENTER.md) supplies the fuller argument, examples, evidence
qualifications, and clickable references so the proposal works without its author.

## Share

The PDF uses final-reveal **pixels rendered by Psychopomp**, not a separate HTML
slide implementation. The native deck remains the animated version.

```sh
cargo build --release
python3 scenes/opencode-quality/export.py
```

The exporter requires Python's Pillow package. Outputs stay in the ignored
`output/opencode-quality/` directory:

- `opencode-quality.pdf`: all 17 slides, 16:9, 1920×1080 source pixels.
- `PRESENTER.md`: companion context and references.
- `contact-sheet.jpg`: all slides for visual inspection.
- `slides/`: individual full-resolution PNGs.

The exporter reads the generated deck; rerun the Scene Program after editing Rust.
PDF pages show every reveal, while the native deck starts with its first beat.
PDF text is rasterized and links live in the companion notes.

## Narrated video

The video uses the same actors and spring profiles as the deck. Reveal events are
retimed to phrases in the generated narration; speaking rate is not approximated
with a uniform slide duration. A short dip separates slides without overlapping
readable text. Clips begin one second into each slide and retain a quiet tail.

```sh
mkdir -p output/opencode-quality/narration
cp scenes/opencode-quality/narration/script.json output/opencode-quality/narration/script.json
2password run --env 'FISH_AUDIO_API_KEY=op://Personal/Fish Audio OpenCode API Key/credential' -- bun scripts/narrate.ts output/opencode-quality/narration/script.json
cargo run -p psychopomp-opencode-quality -- --video output/opencode-quality/narration
cargo run --release -- plan validate output/opencode-quality/reel.json
cargo run --release -- plan render output/opencode-quality/reel.json output/opencode-quality/opencode-quality.mp4 --theme neutral
```

Final narration uses Kit's default Fish Audio voice. `--draft` on the narration
command uses system speech for a timing study; do not present it as the final
voice. Narration audio and word timings stay in ignored `output/`, while the
script and phrase anchors live in `narration/script.json`. No secrets are saved in
the scene. `video.rs` rejects reordered or unsettled beats instead of silently
guessing missing timings.

## Scope and proof

This adds a Scene Program only. It does not alter the engine, OpenCode, GitHub,
Organizer, or the status of any proposed repair. The native presentation needs no
model API calls; the separate video export uses generated narration. Ordinary
prose uses sharp 160 ms reveals; existing
cards and text never move when another beat is introduced. Wires draw only when
the next part of the causal argument appears. Forward, backward, and interrupted
navigation use the renderer's existing deterministic Playback tracks.

Research is bounded and directional. Issue descriptions are reported behavior,
not reproductions performed while making this deck. Existing service-campaign
counts are recorded evidence, not fresh test results. See the last slide and
presenter notes before quoting these numbers elsewhere.

### Authoring validation

- `cargo test --workspace`: passed (default GPU tests remain ignored).
- `cargo fmt --check`: passed.
- Scene-only strict Clippy: passed.
- Workspace strict Clippy: rerun, but blocked by existing constant-size
  `chunks_exact` / `chunks_exact_mut` loops flagged by the current toolchain in
  renderer files. Those files were left untouched; scene-only strict Clippy passes.
- All 17 final-reveal slides rendered on Metal and inspected in a contact sheet;
  representative dense slides and the Stage loop were inspected at full resolution.
- Native slide switching, reveal, retreat, and skip input exercised; the returned
  final poses were inspected. This is not an exhaustive pixel-continuity test.
- A 1.4-second, 1920×1080, 60 fps shutter-sampled Stage study rendered successfully.
  Eight motion frames at 40 ms intervals were inspected.
- PDFKit readback confirmed 17 pages at 960×540 points (16:9).
- All 17 final Fish narration clips generated; the phrase-timed reel validates at
  640.711 seconds. Fifty-one shutter-sampled video frames inspected across all
  sections: initial pose, a reveal in progress, and the final reveal.
- A complete diagram section rendered with narration; its encoded motion frame
  inspected at full resolution. FFprobe confirmed 1920×1080, 60 fps, and AAC;
  measured loudness was -16.16 LUFS with -1.98 dBTP peaks.
- The complete 17-section MP4 rendered on Metal. FFprobe confirmed 1920×1080,
  60 fps, AAC, and 640.716667 seconds (frame-rounded from the reel clock).
  Encoded frames from the beginning, diagrams, decision gate, and ending were
  inspected. Full-film loudness measured -16.43 LUFS with -1.86 dBTP peaks.
  This is artifact and sampled-frame verification, not a complete perceptual
  listening or uninterrupted playback review.

No GPU, font, or FFmpeg limitation prevented artifact verification.
