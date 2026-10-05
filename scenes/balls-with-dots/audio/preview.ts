// The film's sound timeline, keyed to what is said, and a rough preview mix of it.
// Writes audio/timeline.json (every placement plus on-screen markers, in film
// seconds) and output/balls-preview.mp3. The mix mirrors the renderer's encoder:
// per-placement trim, gain, and delay, `amix` without normalization, then the same
// peak limiter. Phase 2's Scene Program ports these cues to `Spoken::at`.
//
// Usage (repository root): bun scenes/balls-with-dots/audio/preview.ts
import path from "node:path"
import { audio, durationNanos, run, scene } from "./lib"

type Word = { word: string; start: number; end: number }
type Placement = { id: string; file: string; role: "script" | "layer"; at: number; from: number; to: number; gainDb: number }
type Marker = { at: number; cue: string; screen: string }

const narration = path.join(scene, "narration")
const beatsFile = await Bun.file(path.join(audio, "beats.json")).json()
const manifest = await Bun.file(path.join(narration, "narration.json")).json()
const placements: Placement[] = []
const markers: Marker[] = []
const round = (seconds: number) => Math.round(seconds * 1000) / 1000
const length = (file: string) => durationNanos(path.join(scene, file)) / 1e9

function layer(id: string, file: string, at: number, gainDb: number, range: { from?: number; to?: number; until?: number } = {}) {
  const from = range.from ?? 0
  let to = Math.min(range.to ?? Infinity, length(file))
  if (range.until !== undefined) to = Math.min(to, from + range.until - at)
  if (to <= from) return
  placements.push({ id, file, role: "layer", at: round(at), from: round(from), to: round(to), gainDb })
}
const mark = (at: number, cue: string, screen: string) => markers.push({ at: round(at), cue, screen })

// A placed narration clip; `at(phrase)` gives film time, like `Spoken::at`.
type Spoken = { id: string; start: number; end: number; voiceEnd: number; words: Word[]; beats: number[] }
async function speak(id: string, start: number, gainDb: number): Promise<Spoken> {
  const entry = manifest.clips.find((clip: { id: string }) => clip.id === id)
  const beats = beatsFile.clips[id]
  if (!entry || beats?.textHash !== entry.textHash) throw new Error(`clip '${id}' changed: rerun beats.ts`)
  const words: Word[] = (await Bun.file(path.join(narration, entry.words)).json()).wordTimings
  const duration = entry.durationNanos / 1e9
  placements.push({ id: `narration-${id}`, file: `narration/${entry.file}`, role: "script", at: round(start), from: 0, to: round(duration), gainDb })
  const last = beats.beats.at(-1)
  return { id, start, end: start + duration, voiceEnd: start + last.end, words, beats: beats.beats.map((beat: { start: number }) => start + beat.start) }
}
const plain = (text: string) => text.toLowerCase().replace(/[^a-z0-9' ]/g, "").trim()
// `after` is film time, like `Spoken::at_after`.
function at(spoken: Spoken, phrase: string, after = 0) {
  const want = plain(phrase).split(/\s+/)
  const words = spoken.words
  for (let i = 0; i + want.length <= words.length; i++)
    if (spoken.start + words[i].start >= after && want.every((word, k) => plain(words[i + k].word) === word)) return spoken.start + words[i].start
  throw new Error(`clip '${spoken.id}': no '${phrase}'`)
}

// Deterministic scatter for the climax (mulberry32).
let seed = 7
function random() {
  seed = (seed + 0x6d2b79f5) | 0
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed)
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296
}

// Act 1: one ball, whispered.
const hush = await speak("hush", 0.5, -2)
layer("room-tone-a", "audio/sfx/room-tone.flac", 0, -26)
layer("mic-rustle-open", "audio/sfx/mic-rustle.flac", 0.05, -16)
mark(0, "open", "black; a faint glow at center")
const firstBall = at(hush, "balls")
layer("pop-ball-1", "audio/sfx/pop-00.flac", firstBall, -12)
layer("sparkle-ball-1", "../../assets/psychopomp-intro/sparkle.wav", firstBall, -20)
mark(firstBall, "hush: balls", "ball 1 fades in")
mark(at(hush, "with dots"), "hush: with dots", "its dots light up, one by one")
mark(at(hush, "just one ball"), "hush: just one ball", "slow push-in on ball 1")

const another = await speak("another", hush.end + 0.35, -2)
layer("room-tone-b", "audio/sfx/room-tone.flac", 10, -26, { until: another.end + 0.15 })
layer("mic-rustle-turn", "audio/sfx/mic-rustle.flac", hush.end - 0.2, -18)
layer("pop-ball-2", "audio/sfx/pop-01.flac", at(another, "another"), -10)
mark(at(another, "another"), "another: another", "ball 2 pops in beside it")
mark(at(another, "more dots"), "another: more dots", "ball 2's dots multiply")
const droneStart = at(another, "another")
layer("drone-swell", "audio/sfx/drone-swell.flac", droneStart, -14)

// Act 2: the chant. One ball per "balls", a semitone higher each time.
const chant = await speak("chant", another.end + 0.15, 0)
chant.beats.forEach((beat, index) => {
  layer(`pop-chant-${index + 1}`, `audio/sfx/pop-${String(Math.min(index + 2, 15)).padStart(2, "0")}.flac`, beat, -11 + index * 0.5)
  mark(beat, `chant: balls #${index + 1}`, `ball ${index + 3} spawns`)
})
// The heart accelerates from "another" to the end of the chant.
// Its thump lands 0.18 s into the file.
for (let time = droneStart + 0.6, beat = 1; time < chant.end; beat++) {
  layer(`heartbeat-${beat}`, "audio/sfx/heartbeat.flac", time - 0.18, -10)
  const progress = (time - droneStart) / (chant.end - droneStart)
  time += 1.05 + (0.38 - 1.05) * progress
}

// Act 3: the theory, rising into the riser and the impact on "ANDY".
const theory = await speak("theory", chant.end + 0.12, 0)
const andyStart = theory.end + 0.05
layer("heartbeat-race", "audio/sfx/heartbeat-race.flac", theory.start, -14, { until: andyStart })
layer("drone", "audio/sfx/drone.flac", droneStart + 15, -14)
layer("zap-atoms", "audio/sfx/zap.flac", at(theory, "atoms"), -14)
mark(at(theory, "atoms"), "theory: atoms", "electrons start orbiting every ball")
layer("glitch-thoughts", "audio/sfx/glitch.flac", at(theory, "thoughts"), -16)
mark(at(theory, "thoughts"), "theory: thoughts", "balls flicker into thought bubbles")
mark(at(theory, "hopes"), "theory: hopes", "they glow warm")
mark(at(theory, "dreams"), "theory: dreams", "they drift upward")
const backToDots = at(theory, "balls with dots", at(theory, "dreams"))
layer("glitch-dots", "audio/sfx/glitch.flac", backToDots, -14)
mark(backToDots, "theory: represent balls with dots", "thought bubbles snap back into dotted balls")
layer("glitch-basketballs", "audio/sfx/glitch.flac", at(theory, "basketballs"), -10)
mark(at(theory, "basketballs"), "theory: basketballs", "every ball is a basketball for a frame")

layer("riser", "audio/sfx/riser.flac", andyStart - length("audio/sfx/riser.flac"), -10)
const andy = await speak("andy", andyStart, 1)
layer("impact-andy", "audio/sfx/impact.flac", andy.start, -2)
layer("glitch-andy", "audio/sfx/glitch.flac", andy.start, -10)
mark(andy.start, "andy: ANDY SERKIS", "IMPACT + quake: the balls slam onto a motion-capture suit")
layer("flyby-anything", "audio/sfx/flyby.flac", at(andy, "anything"), -10)
mark(at(andy, "anything"), "andy: anything", "the suit's markers scatter and re-form")
layer("chimp-cameo", "narration/chaos-chimp.mp3", at(andy, "chimp") + 0.35, -14, { to: 0.75 })
mark(at(andy, "chimp"), "andy: chimp", "the markers form a chimp")
mark(at(andy, "shriveled"), "andy: little shriveled guy", "a small hunched figure")

// Act 4: spaghetti, Will Smith, and every voice at once.
const spaghetti = await speak("spaghetti", andy.end + 0.05, 1)
layer("drone-late", "audio/sfx/drone.flac", droneStart + 30, -14)
layer("whoosh-lasagna", "../../assets/psychopomp-intro/whoosh.wav", at(spaghetti, "lasagna"), -10)
mark(at(spaghetti, "lasagna"), "spaghetti: lasagna", "the figure flattens into lasagna layers")
layer("whoosh-spaghetti", "../../assets/psychopomp-intro/whoosh.wav", at(spaghetti, "spaghetti"), -10)
mark(at(spaghetti, "spaghetti"), "spaghetti: spaghetti", "then unravels into noodles")
mark(at(spaghetti, "woo"), "spaghetti: (howl)", "camera quake")
mark(at(spaghetti, "spaghetti kid"), "spaghetti: spaghetti kid", "a noodle child with ball eyes")
mark(at(spaghetti, "will smith"), "spaghetti: will smith", "a giant fork descends")
// The voices pile in as the final shriek trails off, so "WILL SMITH" stays clear.
const chaos = spaghetti.voiceEnd - 0.15
const cut = chaos + 4.6
mark(chaos, "chaos", "balls fly everywhere; glitch, bloom, quake; voices overlap")
layer("intro-riser", "../../assets/psychopomp-intro/riser.wav", cut - length("../../assets/psychopomp-intro/riser.wav"), -12)
const voices: [string, string, number, number, number?][] = [
  // id, file, offset after the chaos cue, gain, source start
  ["babble", "narration/chaos-babble.mp3", 0.2, -7],
  ["yowl", "narration/chaos-yowl.mp3", 0.35, -6],
  ["laugh", "narration/chaos-laugh.mp3", 0.5, -8],
  ["babble-chipmunk", "audio/voices/babble-chipmunk.mp3", 0.6, -11],
  ["yowl-demon", "audio/voices/yowl-demon.mp3", 0.8, -8],
  ["laugh-demon", "audio/voices/laugh-demon.mp3", 0.9, -8],
  ["howl", "narration/chaos-howl.mp3", 1.0, -7],
  ["spaghetti-demon", "audio/voices/spaghetti-demon.mp3", 1.1, -9, 3.5],
  ["babble-demon", "audio/voices/babble-demon.mp3", 1.2, -9],
  ["laugh-chipmunk", "audio/voices/laugh-chipmunk.mp3", 1.4, -12],
  ["howl-low", "audio/voices/howl-low.mp3", 1.5, -9],
  ["chimp", "narration/chaos-chimp.mp3", 1.7, -9],
  ["theory-reversed", "audio/voices/theory-reversed.mp3", 1.8, -13],
  ["chant-demon", "audio/voices/chant-demon.mp3", 2.0, -9, 4.1],
  ["spaghetti-chipmunk", "audio/voices/spaghetti-chipmunk.mp3", 2.2, -12, 5.8],
  ["babble-reversed", "audio/voices/babble-reversed.mp3", 2.4, -12],
]
for (const [id, file, offset, gainDb, from] of voices) layer(`chaos-${id}`, file, chaos + offset, gainDb, { from, until: cut })
for (let i = 0; i < 9; i++) layer(`chaos-flyby-${i}`, "audio/sfx/flyby.flac", chaos + 0.3 + i * 0.42 + random() * 0.2, -12 - random() * 4, { until: cut })
for (let i = 0; i < 5; i++) layer(`chaos-glitch-${i}`, "audio/sfx/glitch.flac", chaos + 0.1 + i * 0.85 + random() * 0.3, -12, { until: cut })
for (let i = 0; i < 4; i++) layer(`chaos-zap-${i}`, "audio/sfx/zap.flac", chaos + 0.5 + i * 0.95 + random() * 0.3, -13, { until: cut })
for (let i = 0; i < 14; i++) layer(`chaos-pop-${i}`, `audio/sfx/pop-${String(4 + Math.floor(random() * 12)).padStart(2, "0")}.flac`, chaos + random() * (cut - chaos - 0.3), -14, { until: cut })
for (const placement of placements) {
  // Everything stops at the hard cut, beds and drones included.
  if (placement.role === "layer" && placement.at < cut && placement.at + placement.to - placement.from > cut)
    placement.to = round(placement.from + cut - placement.at)
}

// Act 5: silence, a ringing ear, a slurp, and tenderness.
mark(cut, "hard cut", "black; everything stops")
layer("tinnitus", "audio/sfx/tinnitus.flac", cut, -30)
const slurp = cut + 0.45
layer("slurp", "audio/sfx/slurp.flac", slurp, -6)
mark(slurp, "slurp", "off-screen: one noodle is slurped up")
const after = await speak("after", slurp + 1.2, -3)
mark(at(after, "balls"), "after: balls with dots", "one ball, dim, its dots glowing softly")
const duration = round(after.end + 1.2)
mark(duration, "end", "fade out")

placements.sort((a, b) => a.at - b.at)
markers.sort((a, b) => a.at - b.at)
await Bun.write(path.join(audio, "timeline.json"), JSON.stringify({ duration, placements, markers }, null, 1) + "\n")

// Mirror crates/psychopomp-render/src/encode.rs.
const inputs = placements.flatMap((placement) => ["-i", path.join(scene, placement.file)])
const filters = placements.map((placement, index) =>
  `[${index}:a]atrim=start=${placement.from}:end=${placement.to},volume=${placement.gainDb.toFixed(3)}dB,asetpts=PTS-STARTPTS,adelay=${(placement.at * 1000).toFixed(3)}:all=1[a${index}]`)
filters.push(`${placements.map((_, index) => `[a${index}]`).join("")}amix=inputs=${placements.length}:duration=longest:normalize=0,alimiter=limit=0.95:level=0:latency=1,apad=whole_dur=${duration}[aout]`)
const output = "output/balls-preview.mp3"
run(["ffmpeg", "-v", "error", "-y", ...inputs, "-filter_complex", filters.join(";"), "-map", "[aout]", "-t", String(duration), "-c:a", "libmp3lame", "-b:a", "192k", output])
console.log(`${output}: ${duration}s, ${placements.length} placements`)
for (const marker of markers) console.log(`${marker.at.toFixed(2).padStart(6)}  ${marker.cue.padEnd(36)} ${marker.screen}`)
