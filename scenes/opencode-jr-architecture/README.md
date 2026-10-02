# OpenCode Jr architecture

A narrated teaching reel of OpenCode Jr, the Slack bot in `opencode-slack`:
the edge, one durable object per thread, the sandbox workspace, publication
back to Slack, surviving resets, and the shared objects around the core.
Every claim is read from that repository's `AGENTS.md`, `ARCHITECTURE.md`,
and source; the code zoom is `src/ingress.ts`, condensed for display.

```sh
# Draft voice (no credentials), then Kit's Fish voice.
bun scripts/narrate.ts scenes/opencode-jr-architecture/narration/script.json --draft
2password run --env 'FISH_AUDIO_API_KEY=op://…' -- \
  bun scripts/narrate.ts scenes/opencode-jr-architecture/narration/script.json
# Emit, check, review, render.
cargo run -p kinograph-opencode-jr-architecture
cargo run --release -- plan validate scenes/opencode-jr-architecture/opencode-jr-architecture.reel.json
bun scripts/sheet.ts scenes/opencode-jr-architecture/opencode-jr-architecture.reel.json 1:68:2.8 --theme opencode
cargo run --release -- plan render scenes/opencode-jr-architecture/opencode-jr-architecture.reel.json \
  output/opencode-jr-architecture.mp4 --theme opencode
```

Chapters are one module each (`intro`, `edge`, `code`, `session`,
`workspace`, `publish`, `resets`, `around`, `outro`); each is a Stage film keyed
to phrases in its narration clip.
