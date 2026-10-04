# Psychopomp introduces itself

A 44-second Stage film that swings between sweet and screaming: an orb wakes up,
clients plug in, packets fly, the orb is blown up and rewound, and a feature list
ramps a sustained camera quake into a whiteout and the title.

```sh
bun scripts/narrate.ts scenes/psychopomp-intro/narration/script.json   # ELEVENLABS_API_KEY
bun scenes/psychopomp-intro/sfx/generate.ts                             # optional; outputs are committed
cargo run -p psychopomp-intro
cargo run --release -- plan render scenes/psychopomp-intro/psychopomp-intro.reel.json \
  output/psychopomp-intro.mp4 --theme neutral
```

The narration uses an ElevenLabs v4 voice; set `voice` in `narration/script.json`
to your own voice ID, regenerate, and the film re-times itself to the new words.
