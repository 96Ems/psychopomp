import { defineScript, wait } from "opencode-drive"

const viewport = { cols: 120, rows: 36 } as const
const commandFile = new URL(
  "../assets/opencode-hot-reload/fire-the-missiles.md",
  import.meta.url,
)

export default defineScript({
  viewport,
  async setup({ config, fs }) {
    config.autoupdate = false
    await fs.writeFile("README.md", "# Missile Control\n")
  },
  async run({ artifacts, fs, llm, ui }) {
    const started = performance.now()
    const cues: Record<string, number> = {}
    const mark = (name: string) => {
      cues[name] = Math.round(performance.now() - started)
    }

    llm.title(() => "Fire the missiles")

    await ui.waitFor((state) => state.focused.editor)
    await wait(900)
    mark("typing-start")
    for (const character of "/fire") {
      await ui.type(character)
      await wait(105)
    }
    if (await ui.matches("/fire-the-missiles")) {
      throw new Error("fire-the-missiles unexpectedly existed before the live edit")
    }
    mark("command-absent")
    await ui.screenshot("fire-command-absent")
    await wait(900)

    mark("command-created")
    const command = await Bun.file(commandFile).text()
    await fs.writeFile(".opencode/commands/fire-the-missiles.md", command)
    await ui.waitFor("/fire-the-missiles", { timeout: 10_000 })
    mark("command-live")
    await ui.screenshot("fire-command-live")
    await wait(900)

    await ui.enter()
    mark("command-completed")
    await wait(500)
    await ui.enter()
    mark("command-submitted")
    await llm.send(
      llm.pause(450),
      llm.text("Everything is live.", { delay: 45, chunkSize: 4 }),
    )
    await ui.waitFor("Everything is live.")
    await wait(700)
    mark("response-complete")
    await ui.screenshot("fire-response-complete")
    await wait(1_800)

    await Bun.write(
      `${artifacts}/fire-cues.json`,
      `${JSON.stringify({ viewport, cues }, null, 2)}\n`,
    )
  },
})
