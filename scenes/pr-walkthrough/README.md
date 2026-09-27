# PR walkthrough

A narrated reel of five OpenCode background-service pull requests. Each PR gets a
Sequence Diagram that plays the broken behavior and replays the fix in the same
slots, then an editor that animates the change as a diff with Line Marks.

```sh
# Voice (only changed clips are regenerated; --draft uses macOS `say`)
2password run --env 'FISH_AUDIO_API_KEY=op://…' -- bun scripts/narrate.ts scenes/pr-walkthrough/narration/script.json
# Emit, check, and render
cargo run -p kinograph-pr-walkthrough
bun scripts/sheet.ts scenes/pr-walkthrough/pr-walkthrough.reel.json 4,30,60,92 --theme opencode
cargo run --release -- plan render scenes/pr-walkthrough/pr-walkthrough.reel.json output/pr-walkthrough.mp4 --theme opencode
```

- `narration/script.json` is the spoken script, one clip per segment part.
- `src/lib.rs` holds the stories: participants, rows, and the phrase each row
  waits for. Changing a line of narration usually means changing its phrase here.
- `src/diff.rs` turns a unified-diff style list (`keep`, `add(step)`,
  `remove(step)`) into keyed editor snapshots. Code is condensed for display; the
  footer says so.
- The generated `pr-walkthrough.reel.json` is checked in and compared with the
  Scene Program by `cargo test -p kinograph-pr-walkthrough`.
