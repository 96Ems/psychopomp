// Generates the film's sound design into audio/sfx/ with ElevenLabs sound generation.
// Run from the repository root with ELEVENLABS_API_KEY set. Outputs are committed;
// raw downloads and their request IDs are cached under output/balls-with-dots/sfx/.
import { mkdir } from "node:fs/promises"
import path from "node:path"
import { audio, cache, durationNanos, run, samples, scene, writeStems, type Stem } from "./lib"

const quiet = "No voice, no music."
const effects = [
  { id: "room-tone", duration: 10, loop: true, text: `Intimate ASMR room tone recorded with a sensitive microphone in a quiet bedroom: very soft warm hiss, faint air, an occasional tiny fabric rustle. ${quiet}` },
  { id: "mic-rustle", duration: 1.5, text: `Close-up ASMR microphone rustle: fingertips softly brushing and tapping a foam microphone windscreen, a gentle crinkle. ${quiet}` },
  { id: "pop", duration: 0.5, text: `One single soft, round, satisfying bubble pop, short and clean, like a small glowing ball appearing out of thin air. ${quiet}` },
  { id: "drone", duration: 15, text: "A low, unsettling dark ambient drone: deep sub-bass hum with slowly beating, detuned, dissonant tones that swell louder and more menacing over time. No voice, no drums, no melody." },
  { id: "heartbeat", duration: 1, text: `A single human heartbeat: one deep, muffled lub-dub thump, close and intimate. ${quiet}` },
  { id: "heartbeat-race", duration: 10, text: `A human heartbeat that starts slow and calm and steadily accelerates into a frantic racing pulse, deep muffled thumps. ${quiet}` },
  { id: "tinnitus", duration: 4, text: `A high-pitched tinnitus ear ring: a pure sustained electronic whine, like ringing ears after an explosion, slowly fading out. ${quiet}` },
  { id: "glitch", duration: 1.2, text: `Short harsh digital glitch bursts: stuttering data corruption, bit-crushed buzz, rapid clicks and dropouts. ${quiet}` },
  { id: "zap", duration: 1, text: `An electric crackle and zap: a sharp high-voltage arc snapping with buzzing sparks, short decay. ${quiet}` },
  { id: "impact", duration: 3, text: `A huge cinematic impact: a massive deep slam with a sub-bass hit, a metallic crunch, and a long dark rumbling tail. ${quiet}` },
  { id: "riser", duration: 5, text: "A terrifying horror riser: screeching dissonant strings and noise climbing in pitch and intensity, faster and faster, ending abruptly at its loudest peak. No voice, no drums." },
  { id: "flyby", duration: 0.8, text: `A small object zipping past the microphone very fast: a quick whistling whoosh flyby. ${quiet}` },
  { id: "slurp", duration: 1.5, text: `A loud, wet, comically exaggerated slurp of spaghetti noodles being sucked up, ending with a little lip smack. ${quiet}` },
]
// One pop per chanted "balls": a rising ladder, a semitone per ball.
const ladder = 16
const reused = ["riser", "boom", "whoosh", "sparkle"].map((id) => `../../assets/psychopomp-intro/${id}.wav`)

await mkdir(path.join(audio, "sfx"), { recursive: true })
await mkdir(path.join(cache, "sfx"), { recursive: true })

// Peak-match to -6 dBFS. One-shots also lose leading silence and fade their tail;
// loops stay whole so back-to-back placements remain seamless.
function finish(raw: string, file: string, loop: boolean) {
  const pcm = samples(raw)
  let peak = 0
  for (const sample of pcm) peak = Math.max(peak, Math.abs(sample))
  if (peak < 0.0001) throw new Error(`silent ${raw}`)
  const start = loop ? 0 : Math.max(0, pcm.findIndex((sample) => Math.abs(sample) > peak * 0.02) / 48000 - 0.003)
  const duration = pcm.length / 48000 - start
  const shape = loop ? [] : [`atrim=start=${start}:duration=${duration}`, "asetpts=PTS-STARTPTS"]
  const fade = loop ? [] : [`afade=t=out:st=${Math.max(0, duration - 0.06)}:d=0.06`]
  run(["ffmpeg", "-v", "error", "-y", "-i", raw, "-af", [...shape, `volume=${0.5 / peak}`, ...fade].join(","),
    "-ar", "48000", "-ac", "1", "-sample_fmt", "s16", "-c:a", "flac", file])
}

const stems: Stem[] = []
for (const effect of effects) {
  const raw = path.join(cache, "sfx", `${effect.id}-raw.mp3`)
  const meta = path.join(cache, "sfx", `${effect.id}-raw.json`)
  const request = { model_id: "eleven_text_to_sound_v2", text: effect.text, duration_seconds: effect.duration, prompt_influence: 0.6, loop: effect.loop ?? false }
  const cached = (await Bun.file(meta).exists()) ? await Bun.file(meta).json() : undefined
  if (!(await Bun.file(raw).exists()) || JSON.stringify(cached?.request) !== JSON.stringify(request)) {
    console.error(`generating ${effect.id}`)
    const response = await fetch("https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_192", {
      method: "POST",
      headers: { "xi-api-key": process.env.ELEVENLABS_API_KEY!, "Content-Type": "application/json" },
      body: JSON.stringify(request),
      signal: AbortSignal.timeout(120_000),
    })
    if (!response.ok) throw new Error(`${effect.id}: HTTP ${response.status}`)
    await Bun.write(raw, await response.arrayBuffer())
    // Sound generation answers with a trace ID rather than Text to Dialogue's request ID.
    const requestId = response.headers.get("request-id") ?? response.headers.get("x-trace-id") ?? undefined
    await Bun.write(meta, JSON.stringify({ request, requestId }, null, 2))
  }
  const { requestId } = await Bun.file(meta).json()
  const file = path.join(audio, "sfx", `${effect.id}.flac`)
  finish(raw, file, effect.loop ?? false)
  stems.push({ id: effect.id, file: path.relative(scene, file), kind: "sfx", durationNanos: durationNanos(file), source: effect.text, requestId })
}

// Placements have one gain each, so the drone's swell is baked into a variant.
const swell = path.join(audio, "sfx", "drone-swell.flac")
run(["ffmpeg", "-v", "error", "-y", "-i", path.join(audio, "sfx", "drone.flac"), "-af", "afade=t=in:d=13:curve=exp", "-sample_fmt", "s16", "-c:a", "flac", swell])
stems.push({ id: "drone-swell", file: path.relative(scene, swell), kind: "sfx", durationNanos: durationNanos(swell), source: "drone with a 13 s exponential fade-in" })

const pop = path.join(audio, "sfx", "pop.flac")
for (let step = 0; step < ladder; step++) {
  const id = `pop-${String(step).padStart(2, "0")}`
  const file = path.join(audio, "sfx", `${id}.flac`)
  run(["ffmpeg", "-v", "error", "-y", "-i", pop, "-af", `asetrate=48000*${2 ** (step / 12)},aresample=48000`, "-sample_fmt", "s16", "-c:a", "flac", file])
  stems.push({ id, file: path.relative(scene, file), kind: "sfx", durationNanos: durationNanos(file), source: `pop +${step} semitones (resampled)` })
}

for (const file of reused)
  stems.push({ id: `intro-${path.basename(file, ".wav")}`, file, kind: "sfx", durationNanos: durationNanos(path.join(scene, file)), source: "reused from the psychopomp intro" })

await writeStems("sfx", stems)
console.log(stems.map((stem) => `${stem.id} ${(stem.durationNanos / 1e9).toFixed(3)}s`).join("\n"))
