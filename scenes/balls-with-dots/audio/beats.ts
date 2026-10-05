// Syllable beats for narration clips, from the audio itself. Whisper's word timings
// smear and hallucinate on fast repetition ("balls, balls, balls"), so ball spawns
// key off these energy beats instead. A beat starts where the voice rises out of a
// gap; quieter bursts after it ("...lls") belong to the same beat.
//
// Usage (repository root): bun scenes/balls-with-dots/audio/beats.ts
import path from "node:path"
import { audio, samples, scene } from "./lib"

const narration = path.join(scene, "narration")
const rate = 16000
const frame = 160 // 10 ms window
const hop = 40 // 2.5 ms
// Hysteresis below the clip's peak level: a beat starts above `rise` and ends below
// `fall`, so a quieter tail that never rises back above `rise` stays in its beat.
const riseDb = 16
const fallDb = 32

type Beat = { start: number; end: number; peakDb: number }

export function beats(file: string): Beat[] {
  const pcm = samples(file, rate)
  const levels: number[] = []
  for (let at = 0; at + frame <= pcm.length; at += hop) {
    let sum = 0
    for (let i = at; i < at + frame; i++) sum += pcm[i] * pcm[i]
    levels.push(10 * Math.log10(sum / frame + 1e-12))
  }
  const peak = Math.max(...levels)
  const seconds = (index: number) => Math.round(((index * hop + frame / 2) / rate) * 1000) / 1000
  const result: Beat[] = []
  let quiet = 0 // last frame below `fall`
  let open: Beat | undefined
  levels.forEach((level, index) => {
    if (level < peak - fallDb) {
      if (open) result.push(open)
      open = undefined
      quiet = index
    } else if (open) {
      open.end = seconds(index)
      open.peakDb = Math.max(open.peakDb, Math.round(level * 10) / 10)
    } else if (level >= peak - riseDb) open = { start: seconds(quiet + 1), end: seconds(index), peakDb: Math.round(level * 10) / 10 }
  })
  if (open) result.push(open)
  return result
}

if (import.meta.main) {
  const manifest = await Bun.file(path.join(narration, "narration.json")).json()
  const clips: Record<string, { textHash: string; beats: Beat[] }> = {}
  for (const clip of manifest.clips as { id: string; file: string; textHash: string }[]) {
    clips[clip.id] = { textHash: clip.textHash, beats: beats(path.join(narration, clip.file)) }
    console.log(`${clip.id}: ${clips[clip.id].beats.length} beats`, clips[clip.id].beats.map((beat) => beat.start).join(" "))
  }
  await Bun.write(path.join(audio, "beats.json"), JSON.stringify({ clips }, null, 1) + "\n")
}
