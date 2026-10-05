// Derives the climax's extra voices from the narration clips with FFmpeg: pitch
// shifts that keep tempo (formants move with the pitch, so low is demonic and high
// is a chipmunk), slightly detuned doubles, and reversals. Each stays dry so the
// film can place and gain it separately.
//
// Usage (repository root, after scripts/narrate.ts): bun scenes/balls-with-dots/audio/voices.ts
import { mkdir } from "node:fs/promises"
import path from "node:path"
import { audio, durationNanos, run, scene, writeStems, type Stem } from "./lib"

const narration = path.join(scene, "narration")
const variants: { id: string; clip: string; semitones?: number; reverse?: boolean }[] = [
  { id: "spaghetti-demon", clip: "spaghetti", semitones: -6 },
  { id: "spaghetti-chipmunk", clip: "spaghetti", semitones: 7 },
  { id: "spaghetti-double", clip: "spaghetti", semitones: 0.35 },
  { id: "andy-double", clip: "andy", semitones: -0.35 },
  { id: "chant-demon", clip: "chant", semitones: -7 },
  { id: "chant-reversed", clip: "chant", reverse: true },
  { id: "theory-reversed", clip: "theory", reverse: true },
  { id: "babble-demon", clip: "chaos-babble", semitones: -5 },
  { id: "babble-chipmunk", clip: "chaos-babble", semitones: 8 },
  { id: "babble-reversed", clip: "chaos-babble", reverse: true },
  { id: "laugh-demon", clip: "chaos-laugh", semitones: -6 },
  { id: "laugh-chipmunk", clip: "chaos-laugh", semitones: 7 },
  { id: "yowl-demon", clip: "chaos-yowl", semitones: -7 },
  { id: "howl-low", clip: "chaos-howl", semitones: -5 },
]

await mkdir(path.join(audio, "voices"), { recursive: true })
const stems: Stem[] = []
for (const variant of variants) {
  const filters: string[] = []
  if (variant.semitones) {
    const ratio = 2 ** (variant.semitones / 12)
    filters.push(`asetrate=${48000 * ratio}`, "aresample=48000", `atempo=${1 / ratio}`)
  }
  if (variant.reverse) filters.push("areverse")
  const file = path.join(audio, "voices", `${variant.id}.mp3`)
  run(["ffmpeg", "-v", "error", "-y", "-i", path.join(narration, `${variant.clip}.mp3`), "-af", filters.join(","), "-ar", "48000", "-ac", "1", "-b:a", "160k", file])
  const recipe = [variant.semitones ? `${variant.semitones > 0 ? "+" : ""}${variant.semitones} semitones, same tempo` : "", variant.reverse ? "reversed" : ""].filter(Boolean).join(", ")
  stems.push({ id: variant.id, file: path.relative(scene, file), kind: "voice", durationNanos: durationNanos(file), source: `narration/${variant.clip}.mp3: ${recipe}` })
}
await writeStems("voice", stems)
console.log(stems.map((stem) => `${stem.id} ${(stem.durationNanos / 1e9).toFixed(3)}s`).join("\n"))
