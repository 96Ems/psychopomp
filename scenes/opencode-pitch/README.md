# OpenCode quality loops — the pitch

A 78-second narrated motion-graphics pitch, the short companion to the
`opencode-quality` deck. Five Stage films, each keyed to phrases in Kit's
narration:

1. `rented` — an agent learns a bug, ships a fix, the session ends, and the
   bug comes back: the insight is paid for twice.
2. `kept` — the report becomes a failing test, the repair makes it pass, and
   the test is kept; the regression hits the test, not the user.
3. `loops` — replay real failures, stress transitions, check the experience.
4. `pilot` — two weeks, five candidate problems, three lasting proofs, one
   owner, a spending limit, and a decision on day fourteen.
5. `close` — spend tokens once; keep the protection.

The pilot's candidate problems are the clusters named in the deck; the pitch
claims no reproductions, repairs, or measured savings.

```sh
2password run --env 'FISH_AUDIO_API_KEY=op://…' -- \
  bun scripts/narrate.ts scenes/opencode-pitch/narration/script.json
cargo run -p psychopomp-opencode-pitch
cargo run --release -- plan validate scenes/opencode-pitch/opencode-pitch.reel.json
bun scripts/sheet.ts scenes/opencode-pitch/opencode-pitch.reel.json 0:78:2 --theme opencode
cargo run --release -- plan render scenes/opencode-pitch/opencode-pitch.reel.json \
  output/opencode-pitch.mp4 --theme opencode
```
