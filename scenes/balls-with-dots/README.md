# Balls with dots

A reply film: "In the beginning, there were balls." An intimate ASMR whisper
about one ball with dots slides into a chant, a conspiracy theory, Andy Serkis in
a motion-capture suit, spaghetti, Will Smith, and a montage of every voice and
effect over tribal drums, until one colossal hit implodes every ball into a
point. Silence; one small ball; "...balls with dots." The camera pulls back:
the ball is one dot on a bigger ball, and again, to a planet of dots. "In the
end, there were balls."

```sh
cargo run -p psychopomp-balls-with-dots              # writes balls-with-dots.reel.json here
cargo run --release -- plan render scenes/balls-with-dots/balls-with-dots.reel.json output/bwd-raw.mp4 --theme neutral
# Master the delivery: about -16 LUFS with true peaks under -1 dBFS.
ffmpeg -i output/bwd-raw.mp4 -c:v copy -af "aresample=192000,volume=0.8dB,alimiter=limit=0.8:attack=2:release=60:level=disabled,aresample=48000" \
  -c:a aac -b:a 192k -movflags +faststart output/balls-with-dots-v3a.mp4
```

Between beats the camera dives into one dot of a ball and a match cut turns
that dot into the next scene's ball (`Later::dive`, `ReelSegmentPlan::matched_round`),
interleaved with hard cuts, whips, glitches, a held split screen, and a
13-shot montage cut on drum hits; the ending pulls back out the same way.

`src/sound.rs` is the film's clock: it places every clip, sound effect, and
drum by phrase (the later clips sped up by `audio/tempo.ts`, their word timings
and beats divided by the same tempo), and each reel segment takes the sound
inside its window. `audio/timeline.json` and `audio/preview.ts` below describe
the first, slower cut and are kept as the audition record.

```sh
bun --env-file=<.env with ELEVENLABS_API_KEY> scripts/narrate.ts scenes/balls-with-dots/narration/script.json
bun --env-file=<…> scenes/balls-with-dots/audio/sfx.ts   # sound design; outputs are committed
bun scenes/balls-with-dots/audio/voices.ts               # pitched, doubled, and reversed voices
bun scenes/balls-with-dots/audio/beats.ts                # syllable beats from the audio
bun scenes/balls-with-dots/audio/preview.ts              # timeline.json + output/balls-preview.mp3
bun scenes/balls-with-dots/audio/tempo.ts                # chant ×1.15, theory ×1.3, andy ×1.35, spaghetti ×1.4
bun --env-file=<…> scenes/balls-with-dots/audio/drums.ts # tribal drums, cut hits, sub drops, swishes, risers, the ignition
```

Rerun `voices.ts`, `beats.ts`, and `preview.ts` after any clip is regenerated;
`preview.ts` refuses beats from an older take.

## Narration

Voice `8olojUk4IXpvKgaOCHXj`, `eleven_v4`, Text to Dialogue, stability 0.2,
similarity 0.65, `"loudness": "linear"`. Each clip gets one gain toward -16 LUFS
and a peak limiter (they measure -16.5, `hush` -17.4 for its close-mic pops), so
the chant's whisper-to-shout swell survives. Request IDs are in
`narration/narration.json`.

| Clip | Seconds | Whisper transcript (abridged) |
| --- | ---: | --- |
| `hush` | 7.60 | Oh, I hear you like balls with dots. Just one ball with dots on it. |
| `another` | 5.28 | But what if, but what if, there was another ball with more dots? |
| `chant` | 6.88 | Balls. ×13 (Whisper reports 17; see the beats below) |
| `theory` | 9.20 | Balls with dots represent atoms. Balls with thoughts represent hopes and dreams. Balls with thoughts represent balls with dots. Balls with dots represent basketballs. |
| `andy` | 7.44 | Andy Serkis! If you cover Andy Serkis in balls with dots, you can make anything! A chimp! A little shriveled guy! |
| `spaghetti` | 8.48 | He could become lasagna! He could become spaghetti! Woo! (a howl) Andy Serkis is a spaghetti kid! And he's going to be eaten by Will Smith! |
| `chaos-yowl` | 5.04 | Meow! Meow! Meow! |
| `chaos-howl` | 4.88 | Ooooooo… |
| `chaos-laugh` | 7.84 | HAHAHAHA… HAHAHAHA… |
| `chaos-babble` | 7.60 | Balls, dots, dots, balls, dots… (Whisper then loops on "dots") |
| `chaos-chimp` | 6.32 | Woo woo woo woo! AAAAAAAA! … Ah ah ah! … aaaaaah! |
| `after` | 3.44 | (a 1.6 s shaky exhale) Balls with dots. |

No transcript contains a spoken direction tag. `another` stammers "but what if"
twice; that nervous repeat is the performance, so it stays. The chaos clips are
layers, not cue sources: Whisper's words there are unreliable.

Audition evidence (pitch tracking over voiced speech; Kit speaks near 110 Hz):
the whispers are mostly unvoiced (`another` 5 %, `after` 11 %, `hush` 20 % voiced),
and the rant rises in pitch (`theory` median 268 Hz, `andy` 301 Hz, `spaghetti`
397 Hz, the chant's final shout above 800 Hz).

### Retakes

| Clip | Take | Request ID | Verdict |
| --- | --- | --- | --- |
| all | 1 (one-pass dynamic loudness, longer script) | `0JKOxRuLaNFIakfJpm8U` (hush) … | 53.6 s of speech; loudness evened the chant's swell. Replaced. |
| `hush` | 2 | `aNVVwJdgZz2sNguW83vC` | Whispered, but repeated "Just one ball with dots" (12.8 s). |
| `hush` | 3 | `D4xFlECcmH0QgMEBZaHs` | Correct; 28 % voiced. |
| `hush` | 4 | `T863jHznwmRoC6qYDrU1` | Correct; 27 % voiced. |
| `hush` | 5 | `m9BiO4iXANnpjnNGyL54` | **Kept**: correct, 7.6 s, most whispered. |
| `hush` | 6 | `5gMxIDIHbi0iFN6PCQkR` | Repeated the whole script (16.1 s). |

## Chant: one ball per "balls"

Whisper smears fast repetition and invents extra words, so spawns key off
`audio/beats.json`: a beat starts where the voice rises out of a gap, and a
quieter tail (the "-lls") stays in its beat. The 13 beats match the script's
2 slow + 3 faster + 8 rapid-fire "balls".

| Ball | Clip start (s) | Clip end (s) | Film (s) | Pop |
| ---: | ---: | ---: | ---: | --- |
| 3 | 0.113 | 0.725 | 13.99 | `pop-02` |
| 4 | 1.283 | 1.945 | 15.16 | `pop-03` |
| 5 | 2.258 | 2.663 | 16.14 | `pop-04` |
| 6 | 2.893 | 3.250 | 16.77 | `pop-05` |
| 7 | 3.473 | 3.895 | 17.35 | `pop-06` |
| 8 | 4.168 | 4.408 | 18.05 | `pop-07` |
| 9 | 4.460 | 4.673 | 18.34 | `pop-08` |
| 10 | 4.708 | 4.925 | 18.59 | `pop-09` |
| 11 | 4.950 | 5.163 | 18.83 | `pop-10` |
| 12 | 5.193 | 5.415 | 19.07 | `pop-11` |
| 13 | 5.438 | 5.663 | 19.32 | `pop-12` |
| 14 | 5.678 | 5.938 | 19.56 | `pop-13` |
| 15 | 6.023 | 6.613 | 19.90 | `pop-14` |

## Stems

`audio/manifest.json` lists every stem with its exact decoded duration and source.
All are mono 48 kHz, dry, and peak-matched to -6 dBFS (sound effects) or derived
from the -16 LUFS narration (voices), so each placement sets its own gain.

Sound effects (`audio/sfx/*.flac`, `eleven_text_to_sound_v2`, prompt influence 0.6):

| Stem | Seconds | Sound |
| --- | ---: | --- |
| `room-tone` | 10.00 | ASMR bedroom hiss; a seamless loop, untrimmed |
| `mic-rustle` | 1.48 | fingertips on a foam windscreen |
| `pop` | 0.48 | one soft bubble pop |
| `pop-00` … `pop-15` | 0.48 … 0.20 | the pop resampled up a semitone per step |
| `drone` | 15.00 | low detuned drone, steady |
| `drone-swell` | 15.00 | the drone with a 13 s exponential fade-in |
| `heartbeat` | 1.00 | one lub-dub; the thump lands 0.18 s in |
| `heartbeat-race` | 10.00 | a steady fast pulse (the prompt asked for acceleration and did not get it, so the film accelerates single `heartbeat`s instead) |
| `tinnitus` | 4.00 | a 7.85 kHz ring |
| `glitch` | 1.20 | bit-crushed stutter bursts |
| `zap` | 1.00 | electric arc crackle |
| `impact` | 3.00 | deep slam and rumble tail |
| `riser` | 4.64 | horror riser ending at its peak |
| `flyby` | 0.79 | a small object whistling past |
| `slurp` | 1.48 | one wet spaghetti slurp and a lip smack |

Also reused from `assets/psychopomp-intro/`: `riser` 5.67 s, `boom` 3.00 s,
`whoosh` 0.61 s, `sparkle` 1.48 s. The sound effects were generated before
`sfx.ts` captured trace IDs; their exact requests are in `sfx.ts`.

Voices (`audio/voices/*.mp3`, from `voices.ts`; pitch shifts keep tempo, and
formants move with pitch, so down is demonic and up is a chipmunk):

| Stem | Seconds | From |
| --- | ---: | --- |
| `spaghetti-demon` | 8.48 | `spaghetti` -6 semitones |
| `spaghetti-chipmunk` | 8.46 | `spaghetti` +7 |
| `spaghetti-double` | 8.46 | `spaghetti` +0.35 (detuned double) |
| `andy-double` | 7.46 | `andy` -0.35 |
| `chant-demon` | 6.88 | `chant` -7 |
| `chant-reversed` | 6.88 | `chant` reversed |
| `theory-reversed` | 9.20 | `theory` reversed |
| `babble-demon` | 7.60 | `chaos-babble` -5 |
| `babble-chipmunk` | 7.57 | `chaos-babble` +8 |
| `babble-reversed` | 7.60 | `chaos-babble` reversed |
| `laugh-demon` | 7.84 | `chaos-laugh` -6 |
| `laugh-chipmunk` | 7.82 | `chaos-laugh` +7 |
| `yowl-demon` | 5.04 | `chaos-yowl` -7 |
| `howl-low` | 4.88 | `chaos-howl` -5 |

## Timeline

`audio/preview.ts` places everything by phrase, as the Scene Program will, and
writes `audio/timeline.json`: 107 placements (file, film start, source range,
gain) and the markers below. `output/balls-preview.mp3` mixes them as the
renderer's encoder does (`amix` without normalization, then its peak limiter):
56.6 s, -16.8 LUFS integrated, peak -0.7 dBFS.

| Film (s) | Cue | On screen |
| ---: | --- | --- |
| 0.00 | open (room tone, mic rustle) | black; a faint glow at center |
| 0.50 | `hush` starts (-2 dB) | |
| 2.38 | hush: "balls" (pop, sparkle) | ball 1 fades in |
| 3.18 | hush: "with dots" | its dots light up, one by one |
| 4.86 | hush: "just one ball" | slow push-in on ball 1 |
| 8.45 | `another` starts (-2 dB) | |
| 10.83 | another: "another" (pop; drone swell and heartbeat begin) | ball 2 pops in beside it |
| 12.31 | another: "more dots" | ball 2's dots multiply |
| 13.88 | `chant` starts (0 dB); heartbeats 1.0 s → 0.44 s apart | |
| 13.99–19.90 | chant beats 1–13 (rising pop ladder) | balls 3–15 spawn, faster and faster |
| 20.88 | `theory` starts (0 dB, racing pulse) | |
| 21.84 | theory: "atoms" (zap) | electrons start orbiting every ball |
| 23.28 | theory: "thoughts" (glitch) | balls flicker into thought bubbles |
| 23.98 | theory: "hopes" | they glow warm |
| 24.60 | theory: "dreams" | they drift upward |
| 25.48 | riser starts | |
| 26.46 | theory: "represent balls with dots" (glitch) | thought bubbles snap back into dotted balls |
| 28.72 | theory: "basketballs" (glitch) | every ball is a basketball for a frame |
| 30.13 | `andy`: "ANDY SERKIS" (+1 dB; impact, glitch) | impact and quake: the balls slam onto a motion-capture suit |
| 34.63 | andy: "anything" (flyby) | the suit's markers scatter and re-form |
| 35.61 | andy: "chimp" (a chimp hoot) | the markers form a chimp |
| 36.45 | andy: "little shriveled guy" | a small hunched figure |
| 37.62 | `spaghetti` starts (+1 dB) | |
| 38.16 | spaghetti: "lasagna" (whoosh) | the figure flattens into lasagna layers |
| 39.63 | spaghetti: "spaghetti" (whoosh) | then unravels into noodles |
| 40.44 | spaghetti: the howl | camera quake |
| 42.45 | spaghetti: "spaghetti kid" | a noodle child with ball eyes |
| 44.61 | the intro's riser starts, peaking at the hard cut | |
| 44.91 | spaghetti: "will smith" | a giant fork descends |
| 45.68 | chaos: 16 voice layers over 2.4 s, flybys, glitches, zaps, pops | balls fly everywhere; glitch, bloom, quake |
| 50.28 | hard cut: every layer ends; tinnitus at -30 dB | black; everything stops |
| 50.73 | slurp | off-screen, one noodle is slurped up |
| 51.93 | `after` starts (-3 dB) with an exhale | |
| 53.59 | after: "balls with dots" | one ball, dim, its dots glowing softly |
| 56.57 | end | fade out |

The chaos starts as the final "WILL SMITH" trails off (the last beat of
`spaghetti` ends at 8.215 s), so the punchline stays clear.

Notes for the Scene Program:

- Clips keep their phrase cues: `Spoken::at("another")`, `at("atoms")`,
  `at("basketballs")`, `at("will smith")`. Whisper puts `hush`'s first word at
  0.00; its voice starts at 0.14 s. Ball spawns read `audio/beats.json`.
- Every placement has one static gain, so swells are baked into stems
  (`drone-swell`) and the heartbeat accelerates through placement times.
- The encoder mixes mono with no panning; spreading the climax's voices across
  the stereo field would need a pan on media placements.
- The film runs 56.6 s, past the 30–36 s target: the voiced lines alone take
  48 s at this pace. Cutting to the target means dropping lines (for example,
  `theory`'s middle two sentences and the second half of `andy`).
