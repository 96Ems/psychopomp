// Design one ElevenLabs voice per advisor (plus the narrator) and save it to the
// account. Writes narration/voices.json; skips voices it already created.
//   bun --env-file=.env run scenes/high-council/scripts/design-voices.ts
import path from "node:path"

const dir = path.join(import.meta.dir, "..", "narration")
const out = path.join(dir, "voices.json")
const key = process.env.ELEVENLABS_API_KEY
if (!key) throw new Error("ELEVENLABS_API_KEY missing")

const designs: Record<string, { name: string; description: string; text: string }> = {
  simplifier: {
    name: "High Council - Simplifier",
    description:
      "A theatrical British stage actress in her forties playing a ruthless royal butcher in a 1996 CD-ROM game. Crisp, clipped, imperious received pronunciation, sharp consonants, delighted by cutting things. Hammy, over-the-top, studio-recorded 90s FMV voice acting.",
    text: "Sire! Deep in the dungeon sits a twenty-three-file mummy of vendored code! Give me the cleaver, and I shall lop off four thousand lines before noon! Snip, snip, Sire. Nothing pleases me more than an empty folder.",
  },
  performance: {
    name: "High Council - Baron Von Blitz (Performance)",
    description:
      "A frantic, wildly over-caffeinated older male mad scientist with a light German accent, talking extremely fast, breathless and high-energy, voice cracking with urgency. Campy 1996 CD-ROM full motion video actor, theatrical and manic.",
    text: "My Liege! Put your ear to the motherboard! Do you hear your SSD begging for mercy?! Forty-seven transactions per step! Every millisecond counts, Sire, every single one, there is no time, no time at all!",
  },
  "bug-hunter": {
    name: "High Council - Inspector Mandible (Bug Hunter)",
    description:
      "A gravelly, gruff, battle-hardened male army general voice, deep and raspy like he smoked cigars in the trenches, barking orders with grim relish. Hammy 1996 CD-ROM full motion video actor, theatrical and intense.",
    text: "Peer through my loupe, Sire! Three prize beetles in my jar, caught alive in the compaction chamber! I have hunted bugs in forty campaigns, and I have never seen vermin this fat. Let me crush them today!",
  },
  architect: {
    name: "High Council - Architect",
    description:
      "A smug, pompous male Oxford professor in his fifties with a rich, plummy, condescending baritone, savoring every word as if lecturing slow students. Theatrical 1996 CD-ROM full motion video acting.",
    text: "Hear him, Noble Sovereign. One native pipeline, one seam, one deep module, instead of translating every prompt twice. It is, quite frankly, elementary architecture. I did write the textbook, after all.",
  },
  "user-voice": {
    name: "High Council - Tribune Vox (User Voice)",
    description:
      "An earnest, passionate young female town crier, loud and projecting, warm but alarmed, pleading on behalf of the common people. Theatrical 1996 CD-ROM full motion video acting, slightly melodramatic.",
    text: "Sire! The citizens are revolting! Hundreds of them gather at the gates with their issue reports, crying out about compaction and disk writes! Hear the people, Sire, I beg of you, hear the people!",
  },
  contrarian: {
    name: "High Council - Malakor (Contrarian)",
    description:
      "A sly, sneering, velvet-voiced male villain with a silky theatrical baritone, dripping with sarcasm and amusement, like a scheming royal vizier in a campy 1996 CD-ROM full motion video game.",
    text: "I disagree, Sire. Naturally. Sip your coffee slowly. If you let these maniacs loose without me, we will be in Anarchy before breakfast. And do not look at me like that. I am paid either way.",
  },
  narrator: {
    name: "High Council - Narrator",
    description:
      "A warm, friendly, modern American male narrator in his thirties, relaxed and conversational, like a developer showing a fun side project to friends. Clear, natural, lightly amused, not a movie-trailer voice.",
    text: "Imagine waking up to this. Every morning, a handful of AI advisors read the OpenCode repository and its issues, and each one pitches a single idea. Then they argue about it, in character, before you have finished your coffee.",
  },
}

const voices: Record<string, { voiceId: string; name: string; preview: string }> = (await Bun.file(out).exists())
  ? await Bun.file(out).json()
  : {}
const only = Bun.argv.slice(2)

for (const [id, design] of Object.entries(designs)) {
  if (only.length ? !only.includes(id) : voices[id]) continue
  console.error(`designing ${id}`)
  const response = await fetch("https://api.elevenlabs.io/v1/text-to-voice/design?output_format=mp3_44100_128", {
    method: "POST",
    headers: { "xi-api-key": key, "Content-Type": "application/json" },
    body: JSON.stringify({ voice_description: design.description, model_id: "eleven_ttv_v3", text: design.text, guidance_scale: 4 }),
    signal: AbortSignal.timeout(180_000),
  })
  if (!response.ok) throw new Error(`design ${id}: HTTP ${response.status} ${(await response.text()).slice(0, 300)}`)
  const body = (await response.json()) as { previews: { audio_base_64: string; generated_voice_id: string; duration_secs: number }[] }
  body.previews.forEach((preview, index) =>
    Bun.write(path.join(dir, "previews", `${id}-${index}.mp3`), Buffer.from(preview.audio_base_64, "base64")),
  )
  const pick = body.previews[0]
  const created = await fetch("https://api.elevenlabs.io/v1/text-to-voice", {
    method: "POST",
    headers: { "xi-api-key": key, "Content-Type": "application/json" },
    body: JSON.stringify({ voice_name: design.name, voice_description: design.description, generated_voice_id: pick.generated_voice_id }),
  })
  if (!created.ok) throw new Error(`create ${id}: HTTP ${created.status} ${(await created.text()).slice(0, 300)}`)
  const voice = (await created.json()) as { voice_id: string }
  voices[id] = { voiceId: voice.voice_id, name: design.name, preview: `previews/${id}-0.mp3` }
  await Bun.write(out, JSON.stringify(voices, null, 2) + "\n")
}
console.log(JSON.stringify(Object.fromEntries(Object.entries(voices).map(([id, voice]) => [id, voice.name]))))
