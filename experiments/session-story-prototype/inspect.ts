// Artifact inspection only: disposable headless Chromium, never a user's tab.
import { resolve, join } from "node:path"
import { existsSync } from "node:fs"
import { mkdir } from "node:fs/promises"

const root = resolve(import.meta.dir, "../..")
const { chromium } = await import(join(root, "experiments/browser-grid-prototype/node_modules/playwright-core/index.mjs"))
const output = join(root, "output/session-story-prototype/review")
await mkdir(output, { recursive: true })
const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME || [
  "/Users/kit/Library/Caches/ms-playwright/chromium-1234/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
  "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
].find(existsSync) })
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1100 }, deviceScaleFactor: 1, reducedMotion: "reduce" })
  const errors: string[] = []
  page.on("pageerror", (error: Error) => errors.push(error.message))
  await page.goto(`http://127.0.0.1:${process.env.PORT || "5210"}/?prototype=sessions&variant=A`)
  const video = page.locator(".kg-prototype-video")
  await page.waitForFunction(() => (document.querySelector(".kg-prototype-video") as HTMLVideoElement)?.readyState === 4)
  const initial = await video.evaluate((v: HTMLVideoElement) => ({ paused: v.paused, time: v.currentTime }))
  if (!initial.paused || initial.time !== 0) throw new Error("Prototype should not autoplay")
  await page.locator("[data-kinograph-prototype]").scrollIntoViewIfNeeded()
  await page.getByRole("button", { name: "Browse", exact: true }).click()
  await page.waitForFunction(() => { const v = document.querySelector(".kg-prototype-video") as HTMLVideoElement; return !v.seeking && Math.abs(v.currentTime - 9.8) < .01 })
  for (const id of ["A", "B", "C"]) {
    await page.waitForFunction((id: string) => { const v = document.querySelector(".kg-prototype-video") as HTMLVideoElement; return v.currentSrc.endsWith(`/${id}.mp4`) && !v.seeking && v.readyState === 4 && Math.abs(v.currentTime - 9.8) < .01 }, id)
    await page.screenshot({ path: join(output, `${id}-article.png`) })
    if (id !== "C") await page.getByRole("button", { name: "Next variant", exact: true }).click()
  }
  await page.getByRole("button", { name: "Inspect", exact: true }).click()
  await page.waitForFunction(() => { const v = document.querySelector(".kg-prototype-video") as HTMLVideoElement; return !v.seeking && Math.abs(v.currentTime - 13.8) < .01 })
  await page.getByRole("combobox", { name: "Playback speed" }).selectOption("0.25")
  await page.getByRole("button", { name: "Previous variant", exact: true }).click()
  await page.waitForFunction(() => { const v = document.querySelector(".kg-prototype-video") as HTMLVideoElement; return v.currentSrc.endsWith("/B.mp4") && !v.seeking && Math.abs(v.currentTime - 13.8) < .01 && v.playbackRate === .25 })
  await page.screenshot({ path: join(output, "B-detail-article.png") })
  await page.getByRole("button", { name: "Compare current version" }).click()
  await page.locator(".reload-story-workspace").waitFor()
  if (await video.count() !== 0) throw new Error("Current-version comparison did not replace the prototype")
  await page.getByRole("button", { name: "Back to Kinograph" }).click()
  await page.waitForFunction(() => { const v = document.querySelector(".kg-prototype-video") as HTMLVideoElement; return v.readyState === 4 && !v.seeking && Math.abs(v.currentTime - 13.8) < .01 && v.playbackRate === .25 })
  await video.evaluate((v: HTMLVideoElement) => { v.playbackRate = 1; v.currentTime = 8.8; return v.play() })
  await page.waitForFunction(() => (document.querySelector(".kg-prototype-video") as HTMLVideoElement).currentTime > 9.8)
  await video.evaluate((v: HTMLVideoElement) => v.pause())
  if (errors.length) throw new Error(errors.join("\n"))
  await Bun.write(join(output, "report.json"), JSON.stringify({ pass: true, errors, variants: ["A", "B", "C"], sameTimeSwitch: true, quarterSpeed: true, noAutoplay: true, originalComparison: true, liveBrowserTouched: false }, null, 2))
  console.log("PASS: A/B/C in the article, time/speed retained, no autoplay, current-version comparison; no page errors")
} finally {
  await browser.close()
}
