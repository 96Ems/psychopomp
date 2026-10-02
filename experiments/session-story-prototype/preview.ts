// Stage the actual article read-only. Never edit/restart its live dev server.
import { cp, mkdir, mkdtemp, readFile, symlink, writeFile } from "node:fs/promises"
import { resolve, join } from "node:path"

const root = resolve(import.meta.dir, "../..")
const source = resolve(process.env.ARTICLE_REPO || "/Users/kit/code/open-source/opencode-architecture")
const media = join(root, "output/session-story-prototype")
const target = join(root, "target/session-story-prototype")
await mkdir(target, { recursive: true })
const stage = await mkdtemp(join(target, "article-"))
for (const name of ["src", "index.html", "package.json"]) await cp(join(source, name), join(stage, name), { recursive: true })
// Deliberately omit the live checkout's mutable dev services (for example its
// narration writer). This server only renders a snapshot and serves media.
await writeFile(join(stage, "vite.config.ts"), 'import tailwindcss from "@tailwindcss/vite"\nimport react from "@vitejs/plugin-react"\nimport { defineConfig } from "vite"\nexport default defineConfig({ plugins: [tailwindcss(), react()] })\n')
await symlink(join(source, "node_modules"), join(stage, "node_modules"), "dir")
await mkdir(join(stage, "public"), { recursive: true })
// Link only article media directories, not a live browser profile or session log.
for (const name of ["fonts", "demos"]) await symlink(join(source, "public", name), join(stage, "public", name), "dir")
await symlink(media, join(stage, "public/psychopomp-session-prototype"), "dir")
await cp(join(import.meta.dir, "article"), join(stage, "src/experiments/psychopomp-session-prototype"), { recursive: true })
const file = join(stage, "src/ArticlePage.tsx")
let article = await readFile(file, "utf8")
const original = "<HotReloadStory story={chromeDevtoolsStory} />"
if (article.split(original).length !== 2) throw new Error("Article integration point changed; inspect the current source before staging")
article = 'import { PsychopompPrototype } from "./experiments/psychopomp-session-prototype/Preview"\n' + article.replace(original, `<PsychopompPrototype original={${original}} />`)
await writeFile(file, article)
await writeFile(join(target, "latest.json"), JSON.stringify({ stage, source, media, createdAt: new Date().toISOString(), mode: "isolated read-only article copy" }, null, 2))
const port = process.env.PORT || "5210"
console.log(`Psychopomp comparison: http://127.0.0.1:${port}/?prototype=sessions&variant=A`)
console.log(`Isolated article copy: ${stage}`)
const child = Bun.spawn(["bun", join(source, "node_modules/vite/bin/vite.js"), stage, "--host", "127.0.0.1", "--port", port, "--strictPort"], { cwd: stage, stdout: "inherit", stderr: "inherit" })
process.on("SIGINT", () => child.kill("SIGINT"))
process.on("SIGTERM", () => child.kill("SIGTERM"))
process.exit(await child.exited)
