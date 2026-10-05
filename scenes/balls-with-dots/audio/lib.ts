// Shared helpers for the balls-with-dots audio scripts. Run every script from the
// repository root; paths below are relative to it.
import path from "node:path"

export const scene = "scenes/balls-with-dots"
export const audio = path.join(scene, "audio")
export const cache = "output/balls-with-dots"

export type Stem = {
  id: string
  file: string // relative to the scene directory
  kind: "sfx" | "voice" | "drum"
  durationNanos: number
  source: string // prompt, derivation recipe, or reused asset
  requestId?: string
}

export function run(args: string[]) {
  const result = Bun.spawnSync(args, { stdout: "pipe", stderr: "pipe" })
  if (result.exitCode) throw new Error(`${args[0]}: ${result.stderr.toString()}`)
  return result.stdout
}

// Decoded sample count, not container metadata, staying a millisecond inside the audio.
export function durationNanos(file: string) {
  const out = run(["ffprobe", "-v", "error", "-show_entries", "stream=duration_ts,time_base", "-of", "json", file]).toString()
  const stream = JSON.parse(out).streams[0]
  const [num, den] = stream.time_base.split("/").map(Number)
  return Math.floor(((stream.duration_ts * num) / den - 0.001) * 1e9)
}

export function samples(file: string, rate = 48000) {
  const pcm = run(["ffmpeg", "-v", "error", "-i", file, "-ac", "1", "-ar", String(rate), "-f", "f32le", "-"])
  return new Float32Array(pcm.buffer, pcm.byteOffset, pcm.byteLength / 4)
}

// Replace every stem of `kind` in audio/manifest.json, keeping the other kind.
export async function writeStems(kind: Stem["kind"], stems: Stem[]) {
  const manifest = path.join(audio, "manifest.json")
  const kept: Stem[] = (await Bun.file(manifest).exists()) ? (await Bun.file(manifest).json()).stems.filter((stem: Stem) => stem.kind !== kind) : []
  const all = [...kept, ...stems].sort((a, b) => a.kind.localeCompare(b.kind) || a.id.localeCompare(b.id))
  await Bun.write(manifest, JSON.stringify({ stems: all }, null, 2) + "\n")
}
