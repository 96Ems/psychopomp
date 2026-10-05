// Shared helpers for the balls-v3 audio scripts. Run every script from the
// repository root; paths below are relative to it.
import path from "node:path"

export const scene = "scenes/balls-v3"
export const audio = path.join(scene, "audio")
export const cache = "output/balls-v3"

export type Stem = {
  id: string
  file: string // relative to the scene directory
  durationNanos: number
  source: string
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
