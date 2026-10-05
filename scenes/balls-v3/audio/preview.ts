// Mixes a reel's audio exactly as the renderer's encoder does (every media
// placement trimmed, gained, delayed onto the reel clock, `amix` without
// normalization, then its peak limiter), without rendering a frame. Prints
// loudness so the mix can be tuned before a full render.
//
// Usage (repository root): bun scenes/balls-v3/audio/preview.ts [reel.json] [out.wav]
import path from "node:path"
import { run } from "./lib"

const args = Bun.argv.slice(2).filter((arg) => !arg.startsWith("--"))
// --role script|layer mixes only the narration or only everything else.
const role = Bun.argv.includes("--role") ? Bun.argv[Bun.argv.indexOf("--role") + 1] : undefined
const reelPath = args.find((arg) => arg.endsWith(".json")) ?? "scenes/balls-v3/balls-v3.reel.json"
const out = args.find((arg) => arg.endsWith(".wav")) ?? "output/balls-v3-preview.wav"
const reel = await Bun.file(reelPath).json()
const base = path.dirname(reelPath)

type Media = { id: string; path: string; kind: string; role: string; sourceStartNanos: number; sourceEndNanos: number; timelineStartNanos: number; timelineEndNanos: number; gainDb?: number }
const placements: { file: string; from: number; to: number; at: number; gain: number }[] = []
let start = 0
for (const [index, segment] of reel.segments.entries()) {
  if (index > 0) start -= segment.transitionNanos
  for (const media of (segment.plan.media ?? []) as Media[]) {
    if (media.kind !== "audio" || (role && media.role !== role)) continue
    placements.push({
      file: path.join(base, media.path),
      from: media.sourceStartNanos / 1e9,
      to: media.sourceEndNanos / 1e9,
      at: (start + media.timelineStartNanos) / 1e9,
      gain: media.gainDb ?? 0,
    })
  }
  start += segment.plan.durationNanos
}

const inputs = placements.flatMap((placement) => ["-i", placement.file])
const filters = placements.map(
  (placement, index) =>
    `[${index}:a]atrim=start=${placement.from}:end=${placement.to},volume=${placement.gain.toFixed(3)}dB,asetpts=PTS-STARTPTS,adelay=${(placement.at * 1000).toFixed(3)}:all=1[a${index}]`,
)
filters.push(`${placements.map((_, index) => `[a${index}]`).join("")}amix=inputs=${placements.length}:duration=longest:normalize=0,alimiter=limit=0.95:level=0:latency=1[aout]`)
const script = `${out}.filter`
await Bun.write(script, filters.join(";\n"))
run(["ffmpeg", "-v", "error", "-y", ...inputs, "-/filter_complex", script, "-map", "[aout]", "-ar", "48000", "-ac", "2", out])
const stats = Bun.spawnSync(["ffmpeg", "-hide_banner", "-nostats", "-i", out, "-af", "ebur128=peak=true", "-f", "null", "-"], { stderr: "pipe" }).stderr.toString()
const summary = stats.slice(stats.lastIndexOf("Summary:"))
console.log(`${placements.length} placements, ${(start / 1e9).toFixed(2)} s → ${out}`)
console.log(summary.split("\n").filter((line) => /I:|LRA:|Peak:/.test(line)).map((line) => line.trim()).join("  "))
