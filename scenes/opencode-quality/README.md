# OpenCode quality loops — team proposal

A 17-slide **native Psychopomp presentation**, with progressive reveals and three
quiet Stage card-and-beam diagrams. It argues for a bounded pilot, not a new platform
or a claim that the proposed OpenCode repairs are already complete.

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

## Scope and proof

This adds a Scene Program only. It does not alter the engine, OpenCode, GitHub,
Organizer, or the status of any proposed repair. There is no narration or model
API call in the presentation. Ordinary prose uses sharp 160 ms reveals; existing
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
- Workspace strict Clippy: run, but blocked by unrelated unused helpers in
  `render/ui.rs` and the unused `ContentFit::Fill` variant in `render/ui/card.rs`
  during concurrent engine cleanup. Those files were left untouched.
- All 17 final-reveal slides rendered on Metal and inspected in a contact sheet;
  representative dense slides and the Stage loop were inspected at full resolution.
- Native slide switching, reveal, retreat, and skip input exercised; the returned
  final poses were inspected. This is not an exhaustive pixel-continuity test.
- A 1.4-second, 1920×1080, 60 fps shutter-sampled Stage study rendered successfully.
  Eight motion frames at 40 ms intervals were inspected. No claim of a complete
  film playback review is made.
- PDFKit readback confirmed 17 pages at 960×540 points (16:9).

No GPU, font, or FFmpeg limitation prevented artifact verification.
