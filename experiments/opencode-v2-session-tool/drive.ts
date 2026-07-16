import { defineScript, wait } from "opencode-drive"

function requestHasTool(body: unknown, name: string) {
  return JSON.stringify(body).includes(`\"${name}\"`)
}

export default defineScript({
  viewport: { cols: 112, rows: 32 },
  setup({ config }) {
    config.autoupdate = false
  },
  async run({ ui, llm }) {
    llm.title(() => "Create and hot-reload a plugin tool")
    llm.serve(async function* (request, index) {
      const hasTool = requestHasTool(request.body, "session_greeting")
      if (index === 0) {
        if (hasTool) throw new Error("session_greeting existed before the plugin file was created")
        yield llm.reasoning("I’ll create a local v2 plugin that registers the new tool.", {
          delay: 12,
          chunkSize: 8,
        })
        yield llm.toolCall({
          index: 0,
          id: "call_create_plugin",
          name: "patch",
          input: {
            patchText: `*** Begin Patch
*** Add File: .opencode/plugins/session-tool.ts
+import { Plugin } from "@opencode-ai/plugin/v2"
+import { Schema } from "effect"
+
+export default Plugin.define({
+  id: "kinograph.hot-reload-tool",
+  setup: async (ctx) => {
+    await ctx.tool.transform((tools) => {
+      tools.add({
+        name: "session_greeting",
+        options: { codemode: false },
+        description: "Prove that a newly created plugin tool hot-reloaded",
+        input: Schema.Struct({ name: Schema.String }),
+        output: Schema.Struct({ message: Schema.String, sessionID: Schema.String }),
+        execute: async ({ name }, context) => ({
+          message: \`Hot reload worked. Hello, \${name}.\`,
+          sessionID: context.sessionID,
+        }),
+      })
+    })
+  },
+})
*** End Patch`,
          },
        })
        return
      }
      if (index === 1) {
        yield llm.text(
          "The plugin file now exists. OpenCode is watching that directory, so no restart is needed.",
          { delay: 12, chunkSize: 8 },
        )
        return
      }
      if (index === 2) {
        if (!hasTool) throw new Error("session_greeting was not hot-reloaded into the existing session")
        yield llm.reasoning("The new tool is now available in this same session. I’ll call it.", {
          delay: 12,
          chunkSize: 8,
        })
        yield llm.toolCall({
          index: 0,
          id: "call_hot_reloaded_tool",
          name: "session_greeting",
          input: { name: "Ada" },
        })
        return
      }
      if (index === 3) {
        if (!hasTool) throw new Error("session_greeting disappeared after its tool call")
        if (!JSON.stringify(request.body).includes("Hot reload worked. Hello, Ada.")) {
          throw new Error("the hot-reloaded tool result was not returned to the model")
        }
        yield llm.text(
          "Tool result: Hot reload worked. Hello, Ada. Same session, no restart.",
          { delay: 12, chunkSize: 8 },
        )
        return
      }
      throw new Error(`unexpected model request ${index + 1}`)
    })

    await ui.submit(
      "Create a local OpenCode v2 plugin tool named session_greeting. Do not restart this session.",
    )
    await ui.waitFor("no restart is needed", { timeout: 15_000 })
    await ui.screenshot("plugin-created")
    await wait(1_200)

    await ui.submit("Now use the newly hot-reloaded session_greeting tool to greet Ada.")
    await ui.waitFor("Same session, no restart.", { timeout: 15_000 })
    await ui.screenshot("tool-executed")
  },
})
