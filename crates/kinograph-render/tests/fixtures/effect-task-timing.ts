// Regenerate with Bun and the cached UMD bundles from motion-dom@12.42.2 and
// motion-utils@12.39.0. No Effect Institute dependency installation is required.
// bun run <this-file> <motion-dom.dev.js> <motion-utils.js> > effect-task-timing.json
const [domPath, utilsPath] = Bun.argv.slice(2)
if (!domPath || !utilsPath) throw new Error("Pass Motion DOM and Motion Utils UMD bundle paths")

async function bundle(path: string, imports: Record<string, unknown> = {}) {
  const exports = {}
  new Function("exports", "module", "require", await Bun.file(path).text())(
    exports,
    { exports },
    (name: string) => {
      if (!(name in imports)) throw new Error(`Unexpected dependency: ${name}`)
      return imports[name]
    },
  )
  return exports
}

const utils = await bundle(utilsPath)
const { spring } = await bundle(domPath, { "motion-utils": utils }) as {
  spring(options: Record<string, unknown>): { next(ms: number): { value: number } }
}
// Source: Effect Institute Pixi engine/config.ts, motion.ts::springFromLambda,
// and node/NodeController.ts::advanceNodeIconsAndContent/advanceNodeBubble.
const profiles = {
  height: [0.2, 0.5],
  width: [0.35, 0.35],
  icon: [3 / 18, 0],
  resultScale: [0.25, 0.4],
  resultBlur: [0.15, 0],
  bubbleOpacity: [3 / 14, 0],
  bubbleBlur: [3 / 16, 0],
  bubbleY: [0.25, 0.5],
  color: [3 / 12, 0],
} as const
const times = [0, 1 / 60, 2 / 60, 0.05, 5 / 60, 0.1, 0.15, 0.2, 0.25, 0.35, 0.5]
console.log(JSON.stringify({
  source: "Effect Institute Pixi node controller (01a02d20); motion-dom 12.42.2",
  note: "Tight rest tolerances isolate the analytic curve, not frame-dependent rest snapping.",
  times,
  profiles: Object.fromEntries(Object.entries(profiles).map(([name, [visualDuration, bounce]]) => {
    const curve = spring({ keyframes: [0, 1], visualDuration, bounce, restDelta: 1e-8, restSpeed: 1e-8 })
    return [name, { visualDuration, bounce, values: times.map(time => curve.next(time * 1000).value) }]
  })),
}, null, 2))
