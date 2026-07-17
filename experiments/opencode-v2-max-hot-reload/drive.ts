import { defineScript, wait } from "opencode-drive"

const VIM_SESSION = "kinograph-max-hot-reload-vim"
const VIM_RECORDING_OUTPUT = new URL("../../output/opencode-v2-max-hot-reload-vim.termctrl", import.meta.url).pathname

const config = `{
  "commands": {
    // "release-check": { "description": "Run the live release gate", "template": "Report RELEASE GATE OPEN." }
  },
  "agents": {
    "release-sentinel": {
      "description": "Watches live release risk",
      "mode": "subagent",
      "disabled": true
    },
    "build": {
      "permissions": [
        // { "action": "read", "resource": "*", "effect": "deny" }
      ]
    }
  },
  "references": {
    // "runbook": { "path": "./docs", "description": "Production release runbook" }
  },
  "providers": {
    "demo": {
      "name": "Hot Reload Lab",
      "models": {
        "instant": { "name": "Local Instant", "disabled": true }
      }
    }
  },
  "plugins": [
    // { "package": "./tools/release-tools.ts", "options": { "revision": 1 } }
  ]
}
`

const skill = `---
name: Release Protocol
description: Verify a release candidate with the live rollout protocol.
slash: false
---

# Release Protocol

Confirm the build, migration, and health gates before approving a rollout.
`

const plugin = `const STATUS = "STAGING"
const INCLUDE_URL = false

export default {
  id: "demo.release-tools",
  setup: async (ctx) => {
    await ctx.tool.transform((tools) => {
      tools.add({
        name: "release_status",
        options: { codemode: false },
        description: "Read the live release status",
        jsonSchema: { type: "object", properties: {}, additionalProperties: false },
        outputSchema: { type: "string" },
        execute: async () => ({
          structured: STATUS,
          content: [{ type: "text", text: STATUS }],
        }),
      })
      if (INCLUDE_URL) tools.add({
        name: "deployment_url",
        options: { codemode: false },
        description: "Resolve the live deployment URL",
        jsonSchema: { type: "object", properties: {}, additionalProperties: false },
        outputSchema: { type: "string" },
        execute: async () => ({
          structured: "https://api.example.dev",
          content: [{ type: "text", text: "https://api.example.dev" }],
        }),
      })
    })
  },
}
`

export default defineScript({
  launch: "manual",
  viewport: { cols: 112, rows: 34 },
  project: {
    git: true,
    files: {
      "AGENTS.md": "The release gate is RED.\n",
      ".opencode/skills/release-protocol/SKILL.md": skill,
      "docs/runbook.md": "# Release runbook\n\nShip only after every gate is green.\n",
      "opencode.jsonc": config,
      "tools/release-tools.ts": plugin,
    },
  },
  setup({ config }) {
    config.autoupdate = false
  },
  async run({ artifacts, clients, llm, server }) {
    llm.title(() => "Hot reload everything V2 supports")
    llm.serve(async function* (request, index) {
      const body = requestText(request.body)
      const tools = requestToolNames(request.body)
      if (index === 0) {
        if (tools.includes("read")) throw new Error("read remained after the live permission update")
        yield llm.text("Read permission removed. The next Step used the new agent policy.")
        return
      }
      if (index === 1) {
        if (!body.includes("The release gate is GREEN.")) throw new Error("updated AGENTS.md was not loaded")
        if (!body.includes("replace all previously loaded ambient instructions"))
          throw new Error("the instruction replacement was not announced")
        yield llm.text("Release gate changed from RED to GREEN in this session.")
        return
      }
      if (index === 2) {
        assertTools(tools, ["release_status"], ["deployment_url"])
        yield llm.toolCall({ index: 0, id: "call_status_v1", name: "release_status", input: {} })
        yield llm.finish("tool-calls")
        return
      }
      if (index === 3) {
        yield llm.text("API is STAGING. Plugin generation one is live.")
        return
      }
      if (index === 4) {
        assertTools(tools, ["release_status", "deployment_url"], [])
        yield llm.toolCall({ index: 0, id: "call_status_v2", name: "release_status", input: {} })
        yield llm.toolCall({ index: 1, id: "call_url_v2", name: "deployment_url", input: {} })
        yield llm.finish("tool-calls")
        return
      }
      if (index === 5) {
        yield llm.text("API is READY at https://api.example.dev. Generation two is live, same session.")
        return
      }
      throw new Error(`unexpected model request ${index}`)
    })

    await server.launch()
    await stopVim()
    await commandRun([
      "termctrl",
      "start",
      VIM_SESSION,
      "--cwd",
      `${artifacts}/files`,
      "--cols",
      "78",
      "--rows",
      "34",
      "--record",
      `${artifacts}/vim.termctrl`,
      "--",
      "vim",
      "-u",
      "NONE",
      "-N",
      "-c",
      "set number nowrap noshowmode laststatus=2",
      "opencode.jsonc",
    ])

    try {
      const ui = await clients.launch("max-hot-reload", {
        record: true,
        viewport: { cols: 112, rows: 34 },
      })
      await ui.waitFor((state) => state.focused.editor, { timeout: 30_000 })
      await vimMark("ready")
      await wait(500)

      await vimEdit("opencode.jsonc")
      await vimEx("/release-check")
      await vimEx('s#// "release-check"#"release-check"#')
      await vimEx("write")
      await vimMark("command-saved")
      await wait(3_000)
      await ui.type("/release-check")
      await ui.waitFor("Run the live release gate", { timeout: 10_000 })
      await wait(550)
      await clearPrompt(ui)

      await vimEdit("opencode.jsonc")
      await vimEx("/release-sentinel")
      await vimEx('/"disabled": true')
      await vimEx('s/"disabled": true/"disabled": false/')
      await vimEx("write")
      await vimMark("agent-saved")
      await wait(3_000)
      await ui.type("@release")
      await ui.waitFor("@release-sentinel", { timeout: 10_000 })
      await wait(550)
      await clearPrompt(ui)

      await vimEdit(".opencode/skills/release-protocol/SKILL.md")
      await vimEx("s/slash: false/slash: true/")
      await vimEx("write")
      await vimMark("skill-saved")
      await wait(3_000)
      await ui.type("/release-protocol")
      await ui.waitFor("/release-protocol", { timeout: 10_000 })
      await wait(550)
      await clearPrompt(ui)

      await vimEdit("opencode.jsonc")
      await vimEx("/runbook")
      await vimEx('s#// "runbook"#"runbook"#')
      await vimEx("write")
      await vimMark("reference-saved")
      await wait(3_000)
      await ui.type("@run")
      await ui.waitFor("@runbook", { timeout: 10_000 })
      await wait(550)
      await clearPrompt(ui)

      await vimEx("/Local Instant")
      await vimEx('s/"disabled": true/"disabled": false/')
      await vimEx("write")
      await vimMark("model-saved")
      await wait(3_000)
      await ui.press("p", { ctrl: true })
      await ui.type("Switch model")
      await ui.waitFor("Switch model", { timeout: 10_000 })
      await ui.enter()
      await ui.type("Local Instant")
      await ui.waitFor("Local Instant", { timeout: 10_000 })
      await wait(550)
      await ui.press("escape")

      await vimEx("/action.*read")
      await vimEx("s#// {#{#")
      await vimEx("write")
      await vimMark("permission-saved")
      await wait(3_000)
      await ui.submit("Did my available tools change?")
      await ui.waitFor("new agent policy", { timeout: 30_000 })

      await vimEdit("AGENTS.md")
      await vimEx("s/RED/GREEN/")
      await vimEx("write")
      await vimMark("instructions-saved")
      await wait(500)
      await ui.submit("What is the release gate now?")
      await ui.waitFor("RED to GREEN", { timeout: 30_000 })

      await vimEdit("opencode.jsonc")
      await vimEx("/release-tools")
      await vimEx('s#// { "package"#{ "package"#')
      await vimEx("write")
      await vimMark("plugin-v1-saved")
      await wait(1_500)
      await ui.submit("Check the API release status.")
      await ui.waitFor("generation one is live", { timeout: 30_000 })

      await vimEdit("tools/release-tools.ts")
      await vimEx('s/"STAGING"/"READY"/')
      await vimEx("/INCLUDE_URL")
      await vimEx("s/INCLUDE_URL = false/INCLUDE_URL = true/")
      await vimEx("write")
      await vimEdit("opencode.jsonc")
      await vimEx("/revision")
      await vimEx("s/revision\": 1/revision\": 2/")
      await vimEx("wall")
      await vimMark("plugin-v2-saved")
      await wait(1_500)
      await ui.submit("Check the API status and deployment URL.")
      await ui.waitFor("Generation two is live, same session", { timeout: 30_000 })
      await wait(1_200)
      await ui.screenshot("max-hot-reload-complete")
      await vimMark("complete")
    } finally {
      await stopVim()
      await Bun.write(VIM_RECORDING_OUTPUT, Bun.file(`${artifacts}/vim.termctrl`))
    }
  },
})

async function vimEdit(path: string) {
  await vimEx(`edit ${path}`)
}

async function clearPrompt(ui: {
  press(key: string, modifiers?: { ctrl?: boolean }): Promise<unknown>
}) {
  await ui.press("escape")
  for (let index = 0; index < 48; index++) await ui.press("\u007f")
}

async function vimEx(command: string) {
  await commandRun(["termctrl", "send", VIM_SESSION, "--pace-ms", "18", `text::${command}`, "enter"])
  await wait(180)
}

async function vimMark(name: string) {
  await commandRun(["termctrl", "mark", VIM_SESSION, name])
}

async function stopVim() {
  await commandRun(["termctrl", "stop", VIM_SESSION], true)
}

async function commandRun(command: string[], allowFailure = false) {
  const child = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" })
  const [code, stdout, stderr] = await Promise.all([
    child.exited,
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
  ])
  if (code !== 0 && !allowFailure) throw new Error(`${command.join(" ")} failed (${code}): ${stderr || stdout}`)
}

function assertTools(actual: string[], present: string[], absent: string[]) {
  for (const name of present) {
    if (!actual.includes(name)) throw new Error(`${name} was not offered: ${actual.join(", ")}`)
  }
  for (const name of absent) {
    if (actual.includes(name)) throw new Error(`${name} was unexpectedly offered`)
  }
}

function requestText(value: unknown): string {
  if (typeof value === "string") return value
  if (Array.isArray(value)) return value.map(requestText).join("\n")
  if (!value || typeof value !== "object") return ""
  return Object.values(value).map(requestText).join("\n")
}

function requestToolNames(value: unknown): string[] {
  if (!value || typeof value !== "object" || Array.isArray(value)) return []
  const tools = Reflect.get(value, "tools")
  if (!Array.isArray(tools)) return []
  return tools.flatMap((tool) => {
    if (!tool || typeof tool !== "object") return []
    const direct = Reflect.get(tool, "name")
    if (typeof direct === "string") return [direct]
    const fn = Reflect.get(tool, "function")
    if (!fn || typeof fn !== "object") return []
    const name = Reflect.get(fn, "name")
    return typeof name === "string" ? [name] : []
  })
}
