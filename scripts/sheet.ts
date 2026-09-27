// Labeled contact sheet of exact frames from a Scene Plan or reel: the fastest way
// to review choreography before a full render. Frames are single samples (no
// motion blur), rendered with the release binary.
//
// Usage:
//   bun scripts/sheet.ts <plan-or-reel.json> <from:to:step | t1,t2,...> [--theme opencode] [--cols 4] [--out output/sheet.jpg]
import { mkdir, rm } from "node:fs/promises"
import path from "node:path"

const args = Bun.argv.slice(2)
const flag = (name: string, fallback: string) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback)
const positional = args.filter((arg, index) => !arg.startsWith("--") && !args[index - 1]?.startsWith("--"))
const [plan, spec] = positional
if (!plan || !spec) throw new Error("usage: bun scripts/sheet.ts <plan-or-reel.json> <from:to:step | t1,t2,...> [--theme NAME] [--cols N] [--out FILE]")
const theme = flag("--theme", "original")
const cols = Number(flag("--cols", "4"))
const out = flag("--out", "output/sheet.jpg")

const times = spec.includes(":")
  ? (() => {
      const [from, to, step] = spec.split(":").map(Number)
      const list: number[] = []
      for (let t = from; t <= to + 1e-9; t += step) list.push(Math.round(t * 1000) / 1000)
      return list
    })()
  : spec.split(",").map(Number)

const root = path.join(import.meta.dir, "..")
const binary = path.join(root, "target/release/kinograph")
if (Bun.spawnSync(["cargo", "build", "--release", "-q"], { cwd: root, stderr: "inherit" }).exitCode !== 0) throw new Error("build failed")
const frames = path.join(root, "output/sheet-frames")
await rm(frames, { recursive: true, force: true })
await mkdir(frames, { recursive: true })

const files: string[] = []
for (const time of times) {
  const file = path.join(frames, `${time.toFixed(3).padStart(8, "0")}.png`)
  const result = Bun.spawnSync([binary, "plan", "frame", plan, String(time), file, "--theme", theme], { stderr: "pipe" })
  if (result.exitCode !== 0) throw new Error(`frame ${time}: ${result.stderr.toString()}`)
  files.push(file)
}

// Label each tile with its time, then tile at half resolution.
const montage = Bun.spawnSync(
  [
    "magick", "montage",
    ...files.flatMap((file, index) => ["-label", `${times[index].toFixed(2)}s`, file]),
    "-tile", `${cols}x`, "-geometry", "960x540+6+6", "-background", "#111", "-fill", "#ddd",
    "-font", "/System/Library/Fonts/Menlo.ttc", "-pointsize", "22", path.resolve(out),
  ],
  { stderr: "pipe" },
)
if (montage.exitCode !== 0) throw new Error(montage.stderr.toString())
console.log(JSON.stringify({ sheet: path.resolve(out), frames: files.length }))
