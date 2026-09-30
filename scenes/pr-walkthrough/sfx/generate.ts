// Generates the flagship's deletion SFX into assets/pr-walkthrough/. Run from
// the repository root with ELEVENLABS_API_KEY injected; the mark tick is a
// synthesized sine and needs no key. Outputs are committed, so this runs once.
import path from "node:path"

const out = "assets/pr-walkthrough"
const effects = [
  {
    id: "glitch",
    text: "A tiny digital glitch: one very short burst of crackling static and data corruption, with a quick falling electronic blip. Dry, close, quiet UI sound. Under a quarter second of sound, then silence. No voice, no music.",
    duration: 0.5,
    // The visual glitch lasts 80 ms; keep only the first burst.
    limit: 0.2,
  },
  {
    id: "sever",
    text: "A single thin, precise slice: a fast high metallic swipe like a razor cutting paper, with a faint electric edge, then an instant soft tail. Dry, close, subtle interface sound. No voice, no music, no impact.",
    duration: 0.6,
    limit: Infinity,
  },
]

const run = (args: string[]) => {
  const p = Bun.spawnSync(args, { stdout: "pipe", stderr: "pipe" })
  if (p.exitCode) throw new Error(`${args[0]}: ${p.stderr.toString()}`)
  return p.stdout
}

// Trim leading silence, peak-match to -6 dBFS, and fade the tail.
function finish(raw: string, file: string, limit = Infinity) {
  const pcm = run(["ffmpeg", "-v", "error", "-i", raw, "-ac", "1", "-ar", "48000", "-f", "f32le", "-"])
  const samples = new Float32Array(pcm.buffer, pcm.byteOffset, pcm.byteLength / 4)
  let peak = 0
  for (const sample of samples) peak = Math.max(peak, Math.abs(sample))
  if (peak < 0.0001) throw new Error(`silent ${raw}`)
  const start = Math.max(0, samples.findIndex(sample => Math.abs(sample) > peak * 0.02) / 48000 - 0.003)
  const duration = Math.min(limit, samples.length / 48000 - start)
  run(["ffmpeg", "-v", "error", "-y", "-i", raw, "-af",
    `atrim=start=${start}:duration=${duration},asetpts=PTS-STARTPTS,volume=${0.5 / peak},afade=t=out:st=${Math.max(0, duration - 0.06)}:d=0.06`,
    "-ar", "48000", "-ac", "1", file])
  return duration
}

const seconds: Record<string, number> = {}
for (const effect of effects) {
  const raw = path.join("output", `sfx-${effect.id}-raw.mp3`)
  if (!(await Bun.file(raw).exists())) {
    const response = await fetch("https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_192", {
      method: "POST",
      headers: { "xi-api-key": process.env.ELEVENLABS_API_KEY!, "Content-Type": "application/json" },
      body: JSON.stringify({ model_id: "eleven_text_to_sound_v2", text: effect.text, duration_seconds: effect.duration, prompt_influence: 0.6, loop: false }),
      signal: AbortSignal.timeout(120_000),
    })
    if (!response.ok) throw new Error(`${effect.id}: HTTP ${response.status}`)
    await Bun.write(raw, await response.arrayBuffer())
  }
  seconds[effect.id] = finish(raw, path.join(out, `${effect.id}.wav`), effect.limit)
}

// The blog's completion voice, lowered for an error: one sine with an 8 ms
// attack and a short exponential release.
run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i",
  "aevalsrc=0.5*sin(2*PI*520*t)*min(t/0.008\\,1)*exp(-max(t-0.008\\,0)/0.05):s=48000:d=0.3",
  "-ac", "1", path.join(out, "mark.wav")])
seconds.mark = 0.3
console.log(JSON.stringify(seconds))
