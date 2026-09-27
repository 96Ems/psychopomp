// Static local files only. No rendering API, uploads, directory traversal or repo access.
import { join } from "node:path";
const generated = process.env.KINOGRAPH_WEB_ASSETS || join(import.meta.dir, "../../target/browser-grid-site");
const types: Record<string, string> = { js: "text/javascript", html: "text/html", wasm: "application/wasm", json: "application/json", png: "image/png", glyphs: "application/octet-stream" };
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: Number(process.env.PORT || 5201),
  async fetch(request) {
    const url = new URL(request.url);
    const name = url.pathname === "/" ? "index.html" : url.pathname.slice(1);
    if (request.method !== "GET" || !/^(?:pkg\/)?[a-zA-Z0-9_.-]+$/.test(name)) return new Response("Not found", { status: 404 });
    const local = ["index.html", "diagram.html", "probe.js"].includes(name);
    const file = Bun.file(join(local ? import.meta.dir : generated, name));
    if (!await file.exists()) return new Response("Not found", { status: 404 });
    const raw = new Uint8Array(await file.arrayBuffer());
    const compress = request.headers.get("accept-encoding")?.includes("gzip") && !name.endsWith(".png");
    return new Response(compress ? Bun.gzipSync(raw) : raw, { headers: {
      "Content-Type": types[name.split(".").pop()!] || "application/octet-stream", "Cache-Control": "no-cache", "Vary": "Accept-Encoding",
      ...(compress ? { "Content-Encoding": "gzip" } : {}),
    }});
  },
});
console.log(`Local browser grid probe: ${server.url}`);
