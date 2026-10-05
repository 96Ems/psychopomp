// Speeds up the rant as it builds: pitch-preserving `atempo` on the later clips,
// written as lossless FLAC so no codec priming shifts the start. The Scene Program places
// these files and divides the original clips' word timings and beats by `tempo`,
// so the cues stay on the words.
//
// Usage (repository root): bun scenes/balls-with-dots/audio/tempo.ts
import { mkdir } from "node:fs/promises"
import path from "node:path"
import { audio, durationNanos, run, scene } from "./lib"

const narration = path.join(scene, "narration")
const faster = [
  { id: "chant", tempo: 1.15 },
  { id: "theory", tempo: 1.3 },
  { id: "andy", tempo: 1.35 },
  { id: "spaghetti", tempo: 1.4 },
]

const manifest = await Bun.file(path.join(narration, "narration.json")).json()
await mkdir(path.join(audio, "tempo"), { recursive: true })
const clips = []
for (const { id, tempo } of faster) {
  const entry = manifest.clips.find((clip: { id: string }) => clip.id === id)
  if (!entry) throw new Error(`narration has no clip '${id}'`)
  const file = path.join(audio, "tempo", `${id}.flac`)
  run(["ffmpeg", "-v", "error", "-y", "-i", path.join(narration, entry.file), "-af", `atempo=${tempo}`, "-ar", "48000", "-ac", "1", "-sample_fmt", "s16", "-c:a", "flac", file])
  const nanos = durationNanos(file)
  const expected = entry.durationNanos / tempo
  if (Math.abs(nanos - expected) > 30e6) throw new Error(`${id}: ${nanos / 1e9}s, expected ${expected / 1e9}s`)
  clips.push({ id, file: path.relative(scene, file), tempo, textHash: entry.textHash, durationNanos: nanos })
  console.log(`${id} ×${tempo}: ${(entry.durationNanos / 1e9).toFixed(3)}s → ${(nanos / 1e9).toFixed(3)}s`)
}
await Bun.write(path.join(audio, "tempo", "tempo.json"), JSON.stringify({ clips }, null, 2) + "\n")
