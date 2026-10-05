# Balls with dots (v3)

The reply film, rebuilt for intensity: peace, then a build, then an overwhelming
climax, then one small ball. It reuses the `balls-with-dots` narration and
stems and adds its own sound design (`audio/sfx.ts`) and one shouted clip
(`narration/cubes.mp3`).

```sh
cargo run -p psychopomp-balls-v3            # writes balls-v3.reel.json here
cargo run --release -- plan render scenes/balls-v3/balls-v3.reel.json output/bwd-v3-raw.mp4 --theme neutral
# Master: -16 LUFS integrated, true peak -3.9 dBFS, mono to both channels.
ffmpeg -i output/bwd-v3-raw.mp4 -c:v copy -af "aresample=192000,volume=-3.6dB,alimiter=limit=0.8:attack=1:release=50:level=disabled,aresample=48000,pan=stereo|c0=c0|c1=c0" \
  -c:a aac -b:a 256k -movflags +faststart output/balls-with-dots-v3b.mp4
```

```sh
bun --env-file=<.env with ELEVENLABS_API_KEY> scenes/balls-v3/audio/sfx.ts   # sound design; outputs committed
bun --env-file=<…> scripts/narrate.ts scenes/balls-v3/narration/script.json  # "CUBES! ARE! BALLS!"
bun scenes/balls-v3/audio/preview.ts [--role script|layer] [out.wav]       # the encoder's mix, no frames
```

`preview.ts` mixes the reel's audio as the encoder does in about 30 seconds
and prints loudness; `--role script` and `--role layer` split the voice from
everything else to check the voice stays on top until the climax.

## Beat sheet

The film clock comes from `src/sound.rs`; times below are from the final render.
The intensity curve is a ramp, not a staircase: every act adds one more kind
of motion (light, then a camera, then cuts, then cuts on drums).

| Act | Film (s) | Sound | Picture | Into the next |
| --- | ---: | --- | --- | --- |
| 0 Genesis | 0.0–3.3 | room tone, warm pad, a sub swell, a mic rustle | black, drifting out-of-focus motes; "In the beginning, there were balls." in Didot Italic racks into focus, then focus pulls past it to a point of light; the camera flies through the title toward it | one take |
| 1 One ball | 3.3–8.6 | the whisper; an ignition on "balls"; chime on "dots" | the point ignites into a soft, defocused ball with a shock ring; "dots" racks it sharp; "just one ball" is a macro orbit, hand-held | one take |
| 2 Another | 8.6–13.8 | drone swell, whoosh and boom on "another", a heartbeat that races | the camera leans off; a sparse ball pops in and the camera swings to both; "more dots" fills it in under a vertigo dolly zoom | one take |
| 3 Chant | 13.9–19.6 | 13 rising pops, a tom or slap under each, sub drop and taiko on "BALLS!" | five balls drop and bounce on a floor of dots, the rest pop in; each word hangs in space, Didot Italic growing into condensed black; from the third beat the camera cuts to a new angle every beat; "BALLS!" fills the frame | the camera dives through the first ball into one of its dots |
| 4 Theory | 19.6–26.7 | the tribal pulse starts and accelerates; zap, thunder, whips, swish, crowd, riser | a round match turns the dot into the nucleus; ATOMS (dots fly into orbits; FIG. 1 annotations); whip to HOPES & dreams (they float up); whip to = SAME THING (a spinning cycle, VHS rewind, a thought becomes a ball); whip to BASKETBALLS! (dribble, shot, swish, confetti); IT'S ALL CONNECTED: pull back on the board and red string | hard cut on the shout |
| 5 Andy | 26.7–32.0 | braam, sub drop, REC beep, marker pops, war drums | ANDY SERKIS! under a vertigo dolly zoom as markers slam onto the suit and lightning arcs out; T-pose orbit; a glass loupe shows each marker is a ball with dots; ANYTHING!; A CHIMP!; "a little shriveled guy" in tiny italic | cube turns down |
| 6 Spaghetti | 32.0–37.8 | whooshes, booms, thunder, a slurp, choir rising, a chomp on "Smith" | LASAGNA sheets drop and bounce from above; SPAGHETTI noodles draw on; WOO!: a full barrel roll through six bolts; SPAGHETTI KID with ball eyes; a fork spears him; WILL SMITH! | glitch |
| 7 Chaos | 37.8–43.9 | sixteen wailing voices, frenzy drums, a drum and pop on every cut, "CUBES! ARE! BALLS!" each on a taiko, stadium crowd, riser into a suction whoosh | 19 shots cut on drums, 0.42 s down to 0.12 s: a storm, a terminal printing BALLS, ATOMS!, CUBES! ARE! BALLS! (a wall of cubes rounds into balls, confetti), a chart, a gauge past red, the chimp electrified, basketball rain, a shield bursting, lasagna, the fork, the loupe, "dots.", a swarm, BALLS, a wall of balls; then BALLS WITH DOTS FOREVER and every ball converges | the colossal hit implodes everything into one point; one white frame |
| 8 After | 43.9–51.9 | dead silence, a faint ring, a shaky exhale, the whisper, a shimmer, a soft drum, a chime | the point stays lit; it swells into a ball; "…balls with dots."; the pull back shows it is a dot on a ball on a ball on a planet; "In the end, there were balls" in the opening's italic, its period a tiny ball that drops and bounces into place | fade |

Rules kept throughout: subtitles are always burned in (whisper in Didot italic,
then louder faces); at most one full-frame white flash in any second; no
likenesses, only names in type.

## Engine changes this cut needed

- Stage labels and subtitles take a `face` (`crates/psychopomp/src/face.rs`):
  the whisper is Didot Italic, the shouting Helvetica Neue Condensed Black.
- `ReelSegmentPlan::matched_round` carries a ball into a ball (an ellipse,
  not a card): the dive into a dot that becomes the atom's nucleus.
- `PlanBuilder::sort_events` and `drop_events_after_end` let independently
  authored beats share channels and let a short montage shot keep a gesture
  that would settle after its cut.
- A Stage holds up to 128 elements (the chant's balls, words, and motes).
