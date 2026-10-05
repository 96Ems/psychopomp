# Generated media

A twelve-second Stage showroom for `psychopomp-media`. A whispered ElevenLabs
line in Kit's voice names the balls; a Fish Audio chant spawns one, with a
generated pop, on every "balls" it says; then the whisper returns six
semitones down.

```sh
PSYCHOPOMP_MEDIA=plan cargo run -p psychopomp-generated-media   # the delta and its cost; calls nothing
cargo run -p psychopomp-generated-media                         # generates what changed, writes the plan
PSYCHOPOMP_MEDIA=prune cargo run -p psychopomp-generated-media  # deletes orphans and superseded files
cargo run --release -- plan render scenes/generated-media/generated-media.plan.json \
  output/generated-media.mp4 --theme neutral
```

`media.lock.json` records the four resources and `media/` holds their audio,
so a fresh checkout runs without credentials. Edit a line and only it, and
what derives from it, regenerates (`ELEVENLABS_API_KEY` or
`FISH_AUDIO_API_KEY`, from the environment or the nearest `.env`).
