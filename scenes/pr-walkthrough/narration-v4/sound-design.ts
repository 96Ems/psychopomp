// Flagship v4 sound-design study. Run from the repository root after emitting
// output/eleven-v4/reel.json. Generated stems/receipts stay in that output folder.
import path from "node:path"
const root = process.cwd()
const dir = path.join(root, "output/eleven-v4")
const spec = await Bun.file("scenes/pr-walkthrough/narration-v4/sfx.json").json()
const run = (args: string[]) => {
  const p = Bun.spawnSync(args, { stdout: "pipe", stderr: "pipe" })
  if (p.exitCode) throw new Error(`${args[0]}: ${p.stderr.toString()}`)
  return p.stdout.toString()
}
const records = []
for (const effect of spec.effects) {
  const raw = path.join(dir, `${effect.id}-raw.mp3`)
  const receipt = path.join(dir, `${effect.id}-receipt.json`)
  if (!(await Bun.file(raw).exists())) {
    const r = await fetch("https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_192", {
      method: "POST", headers: { "xi-api-key": process.env.ELEVENLABS_API_KEY!, "Content-Type": "application/json" },
      body: JSON.stringify({ model_id: spec.model, text: effect.text, duration_seconds: effect.duration, prompt_influence: 0.5, loop: false }),
      signal: AbortSignal.timeout(120_000),
    })
    if (!r.ok) throw new Error(`SFX ${effect.id}: HTTP ${r.status}`)
    if (!r.headers.get("content-type")?.startsWith("audio/")) throw new Error("SFX response is not audio")
    await Bun.write(raw, await r.arrayBuffer())
    await Bun.write(receipt, JSON.stringify({ model: spec.model, ...effect, requestId: r.headers.get("request-id") }, null, 2))
  }
  // Locate audible onset and peak in decoded mono PCM, then trim and peak-match.
  const pcm = Bun.spawnSync(["ffmpeg", "-v", "error", "-i", raw, "-ac", "1", "-ar", "48000", "-f", "f32le", "-"], {stdout:"pipe",stderr:"pipe"})
  if(pcm.exitCode) throw new Error("SFX decode failed")
  const samples = new Float32Array(pcm.stdout.buffer, pcm.stdout.byteOffset, pcm.stdout.byteLength / 4)
  let peak = 0
  for (const sample of samples) peak = Math.max(peak, Math.abs(sample))
  if (peak < 0.0001) throw new Error("Silent SFX")
  const start = Math.max(0, samples.findIndex(sample => Math.abs(sample) > peak * 0.01) / 48000 - 0.005)
  const duration = samples.length / 48000 - start
  const file = path.join(dir, `${effect.id}.wav`)
  run(["ffmpeg","-v","error","-y","-i",raw,"-af",`atrim=start=${start},asetpts=PTS-STARTPTS,volume=${0.5/peak},afade=t=out:st=${Math.max(0,duration-0.08)}:d=0.08`,"-ar","48000","-ac","1",file])
  records.push({...effect,file,durationNanos:Math.floor((duration-0.002)*1e9),trimmedStart:start})
}
const reel = await Bun.file(path.join(dir,"reel.json")).json()
for (const segment of reel.segments) for(const media of segment.plan.media) {
  const effect = records.find(record => record.replaces.includes(media.id))
  if(!effect) continue
  media.path = effect.file
  media.sourceStartNanos = 0
  media.sourceEndNanos = effect.durationNanos
  media.timelineEndNanos = media.timelineStartNanos + effect.durationNanos
  media.gainDb = effect.gainDb
}
await Bun.write(path.join(dir,"reel-sound.json"),JSON.stringify(reel,null,2)+"\n")
await Bun.write(path.join(dir,"sound-design.json"),JSON.stringify(records,null,2)+"\n")
console.log(JSON.stringify(records.map(({id,durationNanos,trimmedStart})=>({id,seconds:durationNanos/1e9,trimmedStart})),null,2))
