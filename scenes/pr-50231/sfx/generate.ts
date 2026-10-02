// Generate this reel's Sound Effects v2 stems. Run from the repository root with
// ELEVENLABS_API_KEY injected. Raw responses are cached in output/pr-50231/sfx;
// the trimmed, peak-matched WAVs the reel references are written beside this file.
import path from "node:path"

const here = import.meta.dir
const cache = path.join(process.cwd(), "output/pr-50231/sfx")
const spec = await Bun.file(path.join(here, "sfx.json")).json()
const run = (args: string[]) => {
  const p = Bun.spawnSync(args, { stdout: "pipe", stderr: "pipe" })
  if (p.exitCode) throw new Error(`${args[0]}: ${p.stderr.toString()}`)
  return p.stdout
}
const records = []
for (const effect of spec.effects) {
  const raw = path.join(cache, `${effect.id}-raw.mp3`)
  if (!(await Bun.file(raw).exists())) {
    const r = await fetch("https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_192", {
      method: "POST",
      headers: { "xi-api-key": process.env.ELEVENLABS_API_KEY!, "Content-Type": "application/json" },
      body: JSON.stringify({
        model_id: spec.model,
        text: effect.text,
        duration_seconds: effect.duration,
        prompt_influence: 0.5,
        loop: false,
      }),
      signal: AbortSignal.timeout(120_000),
    })
    if (!r.ok) throw new Error(`SFX ${effect.id}: HTTP ${r.status}`)
    if (!r.headers.get("content-type")?.startsWith("audio/")) throw new Error("SFX response is not audio")
    await Bun.write(raw, await r.arrayBuffer())
    await Bun.write(
      path.join(cache, `${effect.id}-receipt.json`),
      JSON.stringify({ model: spec.model, ...effect, requestId: r.headers.get("request-id") }, null, 2),
    )
  }
  // Trim leading silence, match peaks to -6 dBFS, and fade the tail.
  const pcm = run(["ffmpeg", "-v", "error", "-i", raw, "-ac", "1", "-ar", "48000", "-f", "f32le", "-"])
  const samples = new Float32Array(pcm.buffer, pcm.byteOffset, pcm.byteLength / 4)
  const peak = samples.reduce((max, sample) => Math.max(max, Math.abs(sample)), 0)
  if (peak < 0.0001) throw new Error(`Silent SFX ${effect.id}`)
  const start = Math.max(0, samples.findIndex((sample) => Math.abs(sample) > peak * 0.01) / 48000 - 0.005)
  const duration = samples.length / 48000 - start
  const file = path.join(here, `${effect.id}.wav`)
  run([
    "ffmpeg", "-v", "error", "-y", "-i", raw, "-af",
    `atrim=start=${start},asetpts=PTS-STARTPTS,volume=${0.5 / peak},afade=t=out:st=${Math.max(0, duration - 0.08)}:d=0.08`,
    "-ar", "48000", "-ac", "1", file,
  ])
  records.push({ id: effect.id, seconds: Number((duration - 0.002).toFixed(3)), trimmedStart: start })
}
console.log(JSON.stringify(records, null, 2))
