// Sound design for the v3 cut, generated with ElevenLabs sound generation
// (eleven_text_to_sound_v2) into audio/sfx/, plus a few stems derived with
// ffmpeg. One-shots lose their leading silence, so a placement's time is its
// attack; beds stay whole. Every stem is mono 48 kHz, peak-matched to -6 dBFS.
// Raw downloads and their trace IDs are cached under output/balls-v3/sfx/.
//
// Usage (repository root): bun --env-file=<.env with ELEVENLABS_API_KEY> scenes/balls-v3/audio/sfx.ts
import { mkdir } from "node:fs/promises"
import path from "node:path"
import { audio, cache, durationNanos, run, samples, scene, type Stem } from "./lib"

const quiet = "No voice, no music."
const effects: { id: string; duration: number; text: string; bed?: boolean; influence?: number }[] = [
  { id: "taiko", duration: 1.2, text: `One single enormous taiko drum hit, deep and booming, with a short dry room tail. Only one hit. ${quiet}` },
  { id: "tom", duration: 0.7, text: `One single tribal floor tom drum hit, punchy, deep and dry. Only one hit. ${quiet}` },
  { id: "slap", duration: 0.5, text: `One single sharp djembe slap, bright and cracking, very short. Only one hit. ${quiet}` },
  { id: "sub-drop", duration: 2.5, text: `A deep cinematic sub-bass drop: one huge 808 boom whose pitch falls away, felt more than heard, with a long tail. ${quiet}` },
  { id: "braam", duration: 3, text: `A massive cinematic brass braam: an enormous distorted low horn blast like a blockbuster movie trailer, one long hit with a dark tail. No voice.` },
  { id: "whoosh-cut", duration: 0.5, text: `A very fast, sharp cinematic whoosh: air ripping past the microphone, short and punchy. ${quiet}` },
  { id: "swell", duration: 1.5, text: `A reverse cymbal swell rising quickly and ending abruptly at its loudest peak, airy and tense. ${quiet}` },
  { id: "ignite", duration: 2.5, text: `A tiny magical spark igniting into a soft, warm glowing hum: delicate, intimate, with a faint glassy shimmer. ${quiet}` },
  { id: "bounce", duration: 0.6, text: `One single basketball bounce on a hardwood court, in an echoing gym. Only one bounce. ${quiet}` },
  { id: "swish", duration: 1.2, text: `A basketball swishing cleanly through a net. ${quiet}` },
  { id: "choir", duration: 8, bed: true, text: "An epic dark choir chanting ominous syllables in a huge cathedral, swelling to a terrifying climax, like a blockbuster movie trailer. No drums." },
  { id: "chomp", duration: 0.7, text: `A huge cartoonish chomp: a giant mouth biting down hard with a wet crunch. ${quiet}` },
  { id: "beep", duration: 0.5, text: `One clean digital recording beep, like a camera starting to record. ${quiet}` },
  { id: "boom", duration: 1.6, text: `One comedic, deep cinematic boom with heavy reverb, like a famous meme sound effect. One hit. ${quiet}` },
  { id: "pad", duration: 12, bed: true, text: "A soft, warm, slowly evolving ambient synth pad, intimate and full of wonder, like the opening of a nature documentary about the cosmos. No percussion, no voice." },
  { id: "suck", duration: 1, text: `A powerful reverse suction whoosh: everything sucked into a single point, rising fast and ending abruptly. ${quiet}` },
  { id: "chime", duration: 1.5, text: `One single tiny, clear, delicate glass chime, soft and pure. ${quiet}` },
  { id: "crowd", duration: 4, bed: true, text: "A stadium crowd roaring and cheering wildly, screaming and stomping." },
  { id: "thunder", duration: 2, text: `A close, violent thunder crack with electric sizzle. ${quiet}` },
  { id: "zap-short", duration: 0.5, text: `A short electric zap, a crisp high-voltage snap. ${quiet}` },
]

await mkdir(path.join(audio, "sfx"), { recursive: true })
await mkdir(path.join(cache, "sfx"), { recursive: true })

function finish(raw: string, file: string, bed: boolean) {
  const pcm = samples(raw)
  let peak = 0
  for (const sample of pcm) peak = Math.max(peak, Math.abs(sample))
  if (peak < 0.0001) throw new Error(`silent ${raw}`)
  const start = bed ? 0 : Math.max(0, pcm.findIndex((sample) => Math.abs(sample) > peak * 0.05) / 48000 - 0.003)
  const duration = pcm.length / 48000 - start
  const shape = bed ? [] : [`atrim=start=${start}:duration=${duration}`, "asetpts=PTS-STARTPTS"]
  run(["ffmpeg", "-v", "error", "-y", "-i", raw, "-af",
    [...shape, `volume=${0.5 / peak}`, `afade=t=out:st=${Math.max(0, duration - 0.06)}:d=0.06`].join(","),
    "-ar", "48000", "-ac", "1", "-sample_fmt", "s16", "-c:a", "flac", file])
}

async function generate(effect: (typeof effects)[number]) {
  const raw = path.join(cache, "sfx", `${effect.id}-raw.mp3`)
  const meta = path.join(cache, "sfx", `${effect.id}-raw.json`)
  const request = { model_id: "eleven_text_to_sound_v2", text: effect.text, duration_seconds: effect.duration, prompt_influence: effect.influence ?? 0.6, loop: false }
  const cached = (await Bun.file(meta).exists()) ? await Bun.file(meta).json() : undefined
  if (!(await Bun.file(raw).exists()) || JSON.stringify(cached?.request) !== JSON.stringify(request)) {
    console.error(`generating ${effect.id}`)
    let response: Response | undefined
    for (let attempt = 0; attempt < 6; attempt++) {
      response = await fetch("https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_192", {
        method: "POST",
        headers: { "xi-api-key": process.env.ELEVENLABS_API_KEY!, "Content-Type": "application/json" },
        body: JSON.stringify(request),
        signal: AbortSignal.timeout(180_000),
      })
      if (response.status !== 429) break
      await Bun.sleep(4000 * (attempt + 1))
    }
    if (!response?.ok) throw new Error(`${effect.id}: HTTP ${response?.status} ${await response?.text()}`)
    await Bun.write(raw, await response.arrayBuffer())
    const requestId = response.headers.get("request-id") ?? response.headers.get("x-trace-id") ?? undefined
    await Bun.write(meta, JSON.stringify({ request, requestId }, null, 2))
  }
  return { effect, raw, meta }
}

// Two at a time keeps under the account's concurrency limit.
const generated = []
for (let index = 0; index < effects.length; index += 2) {
  generated.push(...(await Promise.all(effects.slice(index, index + 2).map(generate))))
}

const stems: Stem[] = []
for (const { effect, raw, meta } of generated) {
  const { requestId } = await Bun.file(meta).json()
  const file = path.join(audio, "sfx", `${effect.id}.flac`)
  finish(raw, file, effect.bed ?? false)
  stems.push({ id: effect.id, file: path.relative(scene, file), durationNanos: durationNanos(file), source: effect.text, requestId })
}

// Derived stems: a slam played backwards rushes into its own hit, and the
// taiko an octave down and up gives the drum kit a low and a high voice.
const derived = [
  { id: "impact-reversed", from: "../balls-with-dots/audio/sfx/impact.flac", filter: "areverse", source: "balls-with-dots impact, reversed" },
  { id: "taiko-low", from: "audio/sfx/taiko.flac", filter: "asetrate=48000*0.75,aresample=48000", source: "taiko down a fourth (resampled)" },
  { id: "taiko-high", from: "audio/sfx/taiko.flac", filter: "asetrate=48000*1.335,aresample=48000", source: "taiko up a fourth (resampled)" },
  { id: "boom-low", from: "audio/sfx/boom.flac", filter: "asetrate=48000*0.7,aresample=48000", source: "boom down (resampled)" },
]
for (const stem of derived) {
  const file = path.join(audio, "sfx", `${stem.id}.flac`)
  run(["ffmpeg", "-v", "error", "-y", "-i", path.join(scene, stem.from), "-af", stem.filter, "-ac", "1", "-sample_fmt", "s16", "-c:a", "flac", file])
  stems.push({ id: stem.id, file: path.relative(scene, file), durationNanos: durationNanos(file), source: stem.source })
}

// The colossal hit, its sub drop, and the braam as one stem that stops dead
// at the cut to silence (sound.rs: CONVERGE - HIT seconds after the hit) with
// a 25 ms fade, so the hard cut does not click.
const cut = 0.3
const slam = path.join(audio, "sfx", "slam-cut.flac")
run(["ffmpeg", "-v", "error", "-y", "-i", path.join(scene, "../balls-with-dots/audio/sfx/drum-colossal.flac"),
  "-i", path.join(audio, "sfx", "sub-drop.flac"), "-i", path.join(audio, "sfx", "braam.flac"), "-filter_complex",
  `[1:a]volume=-4dB[sub];[2:a]volume=-9dB[braam];[0:a][sub][braam]amix=inputs=3:normalize=0,atrim=duration=${cut},afade=t=out:st=${cut - 0.025}:d=0.025`,
  "-ac", "1", "-ar", "48000", "-sample_fmt", "s16", "-c:a", "flac", slam])
stems.push({ id: "slam-cut", file: path.relative(scene, slam), durationNanos: durationNanos(slam), source: `colossal drum + sub drop -4 dB + braam -9 dB, ${cut} s with a 25 ms fade` })

stems.sort((a, b) => a.id.localeCompare(b.id))
await Bun.write(path.join(audio, "manifest.json"), JSON.stringify({ stems }, null, 2) + "\n")
console.log(stems.map((stem) => `${stem.id} ${(stem.durationNanos / 1e9).toFixed(3)}s`).join("\n"))
