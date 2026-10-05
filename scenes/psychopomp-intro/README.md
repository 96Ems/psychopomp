# Psychopomp introduces itself

A 44-second Stage film that swings between sweet and screaming: an orb wakes up,
clients plug in, packets fly, the orb is blown up and rewound, and a feature list
ramps a sustained camera quake into a whiteout and the title.

```sh
cargo run -p psychopomp-intro                                           # ELEVENLABS_API_KEY only if a line changed
bun scenes/psychopomp-intro/sfx/generate.ts                             # optional; outputs are committed
cargo run --release -- plan render scenes/psychopomp-intro/psychopomp-intro.reel.json \
  output/psychopomp-intro.mp4 --theme neutral
```

The narration is declared in `src/main.rs` and recorded in `media.lock.json`
(see `psychopomp-media`). Its three lines were voiced by `scripts/narrate.ts`
and adopted into the lock, so they stay in `narration/`. Edit a line, or set
`KIT` to your own ElevenLabs voice ID, and the next run regenerates exactly
what changed and re-times the film to the new words.
`PSYCHOPOMP_MEDIA=plan cargo run -p psychopomp-intro` shows what that would
cost without calling anything.
