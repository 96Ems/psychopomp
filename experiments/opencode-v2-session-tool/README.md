# Session Tool Demo

This isolated fixture proves that one live OpenCode v2 session can create a
local plugin tool, observe the plugin hot-reload, and call the new tool without
leaving or restarting the session.

The tested versions are:

- `opencode2 v0.0.0-next-15652`
- `@opencode-ai/plugin@0.0.0-next-15649`
- local `/Users/kit/code/open-source/opencode-drive` at `v0.5.0` (`5f2df7e`)

## What It Proves

The first provider request does not advertise `session_greeting`. OpenCode then
uses its built-in `patch` tool to create `.opencode/plugins/session-tool.ts` in
the interactive session. The plugin registers the tool through
`ctx.tool.transform`:

```ts
await ctx.tool.transform((tools) => {
  tools.add({
    name: "session_greeting",
    // schema and executor omitted
  })
})
```

The Drive script checks four provider requests in one session:

1. The tool is absent before the plugin file exists.
2. The built-in patch call creates the plugin while the session remains open.
3. A later prompt in that session advertises and calls `session_greeting`.
4. The tool result returns to the model with the executing session ID.

The generated recording, screenshots, and server log are stored in
`../../assets/opencode-v2-session-tool/`.
