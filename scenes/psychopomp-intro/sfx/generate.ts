// Generates the intro's sound design into assets/psychopomp-intro/. Run from
// the repository root with ELEVENLABS_API_KEY set. Outputs are committed, so
// this runs once; raw downloads are cached under output/.
import path from "node:path"

const out = "assets/psychopomp-intro"
const effects = [
  {
    id: "riser",
    text: "A tense cinematic riser: rising white noise and a synth swell climbing in pitch and intensity, faster and faster, ending abruptly at its loudest peak. No voice, no drums, no music.",
    duration: 6,
  },
  {
    id: "boom",
    text: "A massive cinematic sub boom: a deep distorted bass drop and a crunchy explosion, then a long rumbling tail. No voice, no music.",
    duration: 3,
  },
  {
    id: "whoosh",
    text: "One fast, aggressive whoosh swipe of air rushing past the microphone. Short and dry. No voice, no music.",
    duration: 0.7,
  },
  {
    id: "sparkle",
    text: "A soft, sweet, magical sparkle: gentle tiny twinkling bells, warm and delicate. No voice, no music.",
    duration: 1.5,
  },
]

const run = (args: string[]) => {
  const p = Bun.spawnSync(args, { stdout: "pipe", stderr: "pipe" })
  if (p.exitCode) throw new Error(`${args[0]}: ${p.stderr.toString()}`)
  return p.stdout
}

// Trim leading silence, peak-match to -6 dBFS, and fade the tail.
function finish(raw: string, file: string) {
  const pcm = run(["ffmpeg", "-v", "error", "-i", raw, "-ac", "1", "-ar", "48000", "-f", "f32le", "-"])
  const samples = new Float32Array(pcm.buffer, pcm.byteOffset, pcm.byteLength / 4)
  let peak = 0
  for (const sample of samples) peak = Math.max(peak, Math.abs(sample))
  if (peak < 0.0001) throw new Error(`silent ${raw}`)
  const start = Math.max(0, samples.findIndex((sample) => Math.abs(sample) > peak * 0.02) / 48000 - 0.003)
  const duration = samples.length / 48000 - start
  run(["ffmpeg", "-v", "error", "-y", "-i", raw, "-af",
    `atrim=start=${start}:duration=${duration},asetpts=PTS-STARTPTS,volume=${0.5 / peak},afade=t=out:st=${Math.max(0, duration - 0.06)}:d=0.06`,
    "-ar", "48000", "-ac", "1", file])
  return duration
}

const seconds: Record<string, number> = {}
for (const effect of effects) {
  const raw = path.join("output", `intro-sfx-${effect.id}-raw.mp3`)
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
  seconds[effect.id] = finish(raw, path.join(out, `${effect.id}.wav`))
}
console.log(JSON.stringify(seconds))
