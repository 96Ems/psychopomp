// Tribal drums for the second half and a shimmer for the ending, generated
// with ElevenLabs sound generation like sfx.ts. One-shots lose their leading
// silence so a placement's time is its downbeat; beds stay whole.
//
// Usage (repository root): bun --env-file=<.env with ELEVENLABS_API_KEY> scenes/balls-with-dots/audio/drums.ts
import { mkdir } from "node:fs/promises"
import path from "node:path"
import { audio, cache, durationNanos, run, samples, scene, writeStems, type Stem } from "./lib"

const dry = "No melody, no voice, no music other than percussion."
const drums = [
  { id: "drum-pulse", duration: 8, text: `A sparse, low ritual drum pulse: one deep tribal frame drum struck slowly and steadily, about 100 beats per minute, ominous, cavernous. ${dry}` },
  { id: "drum-war", duration: 12, text: `Building tribal war drums: taiko, djembe, and floor toms in an aggressive pounding rhythm that steadily accelerates and grows louder and more frantic. ${dry}` },
  { id: "drum-frenzy", duration: 6, text: `A full-frenzy tribal drum climax: a thunderous rapid drum roll on many taiko and tom drums at once, chaotic, relentless, at maximum intensity. ${dry}` },
  { id: "drum-colossal", duration: 4, text: `One single colossal drum hit: an enormous taiko and bass drum struck once together, a huge low boom with a long echoing tail. ${dry}` },
  { id: "drum-soft", duration: 3, text: `One single soft, deep, warm drum hit, gently struck, intimate and quiet, with a short natural decay. ${dry}` },
  { id: "shimmer", duration: 5, text: "A soft rising shimmer: delicate airy chimes and a gentle swelling glow that slowly rises in pitch, calm, weightless, magical. No voice, no drums." },
  // One-shots the film sequences itself, so every hit lands on a cut.
  { id: "hit-taiko", duration: 1.5, text: `One single huge taiko drum hit, deep and booming, dry and punchy with a short tail. ${dry}` },
  { id: "hit-tom", duration: 1, text: `One single low floor tom drum hit, tight and punchy, tribal. ${dry}` },
  { id: "hit-slap", duration: 0.6, text: `One single sharp djembe slap, a high cracking hand drum hit, very short. ${dry}` },
  { id: "sub-drop", duration: 2, text: "A cinematic sub-bass drop: a deep booming low-frequency hit that falls in pitch, felt more than heard. No voice, no music." },
  { id: "swish", duration: 0.5, text: "One very fast whoosh swish, like a camera whip pan, short and airy. No voice, no music." },
  { id: "suck", duration: 1.2, text: "A reversed whoosh that rises and gets sucked in fast, ending abruptly at its loudest point, like diving into something. No voice, no music." },
  { id: "riser-short", duration: 2.5, text: "A short intense cinematic riser: noise and tension rising quickly in pitch, ending abruptly at its peak. No voice, no drums." },
  { id: "ignite", duration: 2.5, text: "A soft magical ignition: a tiny spark that blooms into a warm glowing hum with a gentle airy whoosh, delicate and beautiful. No voice, no drums." },
]

function finish(raw: string, file: string, keepStart = false) {
  const pcm = samples(raw)
  let peak = 0
  for (const sample of pcm) peak = Math.max(peak, Math.abs(sample))
  if (peak < 0.0001) throw new Error(`silent ${raw}`)
  const start = keepStart ? 0 : Math.max(0, pcm.findIndex((sample) => Math.abs(sample) > peak * 0.05) / 48000 - 0.004)
  const duration = pcm.length / 48000 - start
  run(["ffmpeg", "-v", "error", "-y", "-i", raw, "-af",
    [`atrim=start=${start}:duration=${duration}`, "asetpts=PTS-STARTPTS", `volume=${0.5 / peak}`, `afade=t=out:st=${Math.max(0, duration - 0.08)}:d=0.08`].join(","),
    "-ar", "48000", "-ac", "1", "-sample_fmt", "s16", "-c:a", "flac", file])
}
// A suck-in or riser peaks at its end, so keep its whole build; only a hit's
// lead-in is trimmed.
const builds = new Set(["suck", "riser-short"])

await mkdir(path.join(audio, "sfx"), { recursive: true })
await mkdir(path.join(cache, "sfx"), { recursive: true })
const generated = []
for (const drum of drums) {
  const raw = path.join(cache, "sfx", `${drum.id}-raw.mp3`)
  const meta = path.join(cache, "sfx", `${drum.id}-raw.json`)
  const request = { model_id: "eleven_text_to_sound_v2", text: drum.text, duration_seconds: drum.duration, prompt_influence: 0.6, loop: false }
  const cached = (await Bun.file(meta).exists()) ? await Bun.file(meta).json() : undefined
  if (!(await Bun.file(raw).exists()) || JSON.stringify(cached?.request) !== JSON.stringify(request)) {
    console.error(`generating ${drum.id}`)
    let response: Response | undefined
    // One request at a time; back off when the account is busy.
    for (let attempt = 0; attempt < 6; attempt++) {
      response = await fetch("https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_192", {
        method: "POST",
        headers: { "xi-api-key": process.env.ELEVENLABS_API_KEY!, "Content-Type": "application/json" },
        body: JSON.stringify(request),
        signal: AbortSignal.timeout(180_000),
      })
      if (response.status !== 429) break
      await Bun.sleep(5000 * (attempt + 1))
    }
    if (!response?.ok) throw new Error(`${drum.id}: HTTP ${response?.status}`)
    await Bun.write(raw, await response.arrayBuffer())
    const requestId = response.headers.get("request-id") ?? response.headers.get("x-trace-id") ?? undefined
    await Bun.write(meta, JSON.stringify({ request, requestId }, null, 2))
  }
  generated.push({ drum, raw, meta })
}
const stems: Stem[] = []
for (const { drum, raw, meta } of generated) {
  const { requestId } = await Bun.file(meta).json()
  const file = path.join(audio, "sfx", `${drum.id}.flac`)
  finish(raw, file, builds.has(drum.id))
  stems.push({ id: drum.id, file: path.relative(scene, file), kind: "drum", durationNanos: durationNanos(file), source: drum.text, requestId })
}
await writeStems("drum", stems)
console.log(stems.map((stem) => `${stem.id} ${(stem.durationNanos / 1e9).toFixed(3)}s`).join("\n"))
