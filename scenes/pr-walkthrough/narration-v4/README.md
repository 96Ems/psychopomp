# Eleven v4 edition

`script.json` directs the same three flagship passages with an expressive arc:
conversational setup, concern at the failure, deliberate emphasis at SIGTERM,
then a firm explanation and understated relief. Stability 0.5 / similarity 0.7
are a starting performance choice, not maximum slider values.

The `Kit Langton` Professional Voice Clone accepts `eleven_v4`. The provider's
voice metadata did not report completed v4-specific fine-tuning during this
study, even after accepting a training request. Successful synthesis is verified;
model-specific training completion is not.

Generate from the repository root:

```sh
mkdir -p output/eleven-v4/narration
cp scenes/pr-walkthrough/narration-v4/script.json output/eleven-v4/narration/script.json
# Inject ELEVENLABS_API_KEY with 2password for each synthesis command.
bun scripts/narrate.ts output/eleven-v4/narration/script.json
cargo run -p kinograph-pr-walkthrough -- pr-50825 --narration output/eleven-v4/narration --output output/eleven-v4/reel.json
bun scenes/pr-walkthrough/narration-v4/sound-design.ts
cargo run --release -- plan render output/eleven-v4/reel-sound.json output/pr-50825-eleven-v4.mp4 --theme neutral
```

The four prompts in `sfx.json` use **Sound Effects v2**, independently of v4
speech: pressure impact, combustion tail, rewind, and resolution. Separate stems
preserve exact cue placement and gain control. The study trims leading silence,
matches peaks to −6 dBFS before authored gains, and fades each tail. Existing
quiet connection and request ticks remain in the mix. To try a different SFX
prompt, remove that study's cached `*-raw.mp3` first; the cache is per effect ID.

The first directed takes produced 22.079 / 12.239 / 9.839-second clips. Whisper
confirmed every spoken phrase and no delivery-tag leakage; the new word timings
drive the Scene Plan. `plan steps` at the actual code snapshot times reports no
stability warnings. The resulting film is about 51.9 seconds, versus 62.2 seconds
for the Fish version. Both exports remain available for listening comparison.

The 1.4-second `post.rewind` shader travels upward in smooth scan bands while the
Burst clock runs backward. The 23.9–25.9 s encoded study was inspected in 40 ms
frames. GPU assertions cover unchanged pixels at age 0 and 1.4, visible activity
between, and identical results after out-of-order sampling. New speech identity
and performance still need human listening judgment; transcription cannot prove
that a clone sounds natural.
