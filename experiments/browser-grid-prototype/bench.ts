// Separate Chrome process/profile; never attaches to the user's live browser.
import { chromium } from "playwright-core";
import { mkdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { resolve, join } from "node:path";
import { captureRun } from "./proof-run";

const label = process.argv[2] || "baseline";
if (!/^[a-z0-9-]+$/.test(label)) throw new Error("use a simple benchmark label");
const assets = resolve(process.env.BENCH_ASSETS || join(import.meta.dir, "../../target/browser-perf-site"));
const control = process.env.BENCH_COMPARE ? resolve(process.env.BENCH_COMPARE) : undefined;
const output = resolve(import.meta.dir, "../../output/browser-perf", label);
await mkdir(output, { recursive: true });
const capture = await captureRun(assets, output, "grid", control ? ["", "control-"] : [""], control);
const server = Bun.serve({
  hostname: "127.0.0.1", port: 0,
  async fetch(request) {
    let path = new URL(request.url).pathname.slice(1);
    const root = path.startsWith("control/") ? control : assets;
    if (path.startsWith("control/")) path = path.slice(8);
    if (!root) return new Response("Not found", { status: 404 });
    if (!path) return new Response('<!doctype html><title>Isolated grid benchmark</title><link rel="icon" href="data:,"><canvas width="1920" height="1080"></canvas>', { headers: { "Content-Type": "text/html" } });
    if (!/^(?:pkg\/)?[a-z0-9_.-]+$/i.test(path)) return new Response("Not found", { status: 404 });
    const file = Bun.file(join(root, path));
    if (!await file.exists()) return new Response("Not found", { status: 404 });
    return new Response(file, { headers: { "Content-Type": path.endsWith(".wasm") ? "application/wasm" : path.endsWith(".js") ? "text/javascript" : file.type } });
  },
});
const browser = await chromium.launch({
  executablePath: process.env.CHROME || ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser"].find(existsSync),
  headless: true,
  args: ["--enable-gpu"],
});
try {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  const errors: string[] = [];
  page.on("pageerror", error => errors.push(String(error)));
  page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
  await page.goto(server.url.toString());
  const report = await page.evaluate(async (compare) => {
    // Requested explicit GPU textures, not physical VRAM or canvas swapchain size.
    const textures: Record<string, { bytes: number; resources: object[] }> = {};
    let allocating = "candidate";
    const owners = new WeakMap<GPUDevice, string>();
    const createTexture = GPUDevice.prototype.createTexture;
    GPUDevice.prototype.createTexture = function(descriptor) {
      const texture = createTexture.call(this, descriptor);
      const owner = owners.get(this) || allocating;
      owners.set(this, owner);
      const size = descriptor.size;
      const [width, height = 1, depth = 1] = Array.isArray(size) ? size : [size.width, size.height, size.depthOrArrayLayers];
      const bytesPerTexel = ({ rgba8unorm: 4, "rgba8unorm-srgb": 4, rgba16float: 8, depth32float: 4, r8unorm: 1 })[descriptor.format];
      if (!bytesPerTexel || (descriptor.mipLevelCount || 1) !== 1) throw new Error("Unaccounted texture format/mips");
      const bytes = width * height * depth * (descriptor.sampleCount || 1) * bytesPerTexel;
      const total = textures[owner] ||= { bytes: 0, resources: [] };
      total.bytes += bytes;
      total.resources.push({ label: descriptor.label, format: descriptor.format, size: [width, height, depth], samples: descriptor.sampleCount || 1, bytes });
      return texture;
    };
    const { default: init, GridCanvas } = await import("/pkg/kinograph_browser_grid_prototype.js");
    const wasm = await init();
    const [plan, glyphs] = await Promise.all([
      fetch("/growing-grid.json").then(r => r.text()),
      fetch("/labels.glyphs").then(r => r.arrayBuffer()).then(b => new Uint8Array(b)),
    ]);
    const adapter = await navigator.gpu?.requestAdapter();
    if (!adapter) throw new Error("No WebGPU adapter in isolated Chrome; no software fallback allowed");
    const info = adapter.info;
    if (/swiftshader|software/i.test(`${info.vendor} ${info.architecture} ${info.description}`)) throw new Error("Software adapter is not a valid GPU baseline");
    const canvas = document.querySelector("canvas")!;
    const engine = await GridCanvas.create(canvas, plan, glyphs);
    const engines = [{ variant: "candidate", engine, canvas }];
    if (compare) {
      const old = await import("/control/pkg/kinograph_browser_grid_prototype.js");
      await old.default();
      const otherCanvas = document.createElement("canvas");
      otherCanvas.width = 1920; otherCanvas.height = 1080;
      document.body.append(otherCanvas);
      allocating = "control";
      engines.push({ variant: "control", engine: await old.GridCanvas.create(otherCanvas, plan, glyphs), canvas: otherCanvas });
    }
    // Only this isolated benchmark page exposes its state for artifact capture.
    (window as any).bench = { engine, canvas, plan, wasm, engines };
    const runs = [];
    for (let run = 0; run < 8; run++) {
      for (const { variant, engine } of run % 2 ? [...engines].reverse() : engines) {
      engine.load(plan);
      engine.draw(0);
      await engine.completed();
      const frames = 240;
      let cpuMs = 0, navigationMs = 0;
      const started = performance.now();
      for (let frame = 0; frame < frames; frame++) {
        const now = frame * 1000 / 60;
        if (frame % 15 === 0) {
          const start = performance.now();
          engine.action(["last", "previous", "last", "first"][Math.floor(frame / 15) % 4], now);
          navigationMs += performance.now() - start;
        }
        const start = performance.now();
        engine.draw(now);
        cpuMs += performance.now() - start;
        // Bound the GPU queue; include completion of every 12-frame batch.
        if (frame % 12 === 11) await engine.completed();
      }
      const elapsedMs = performance.now() - started;
      runs.push({ variant, run, warmup: run === 0, frames, elapsedMs, completedMsPerFrame: elapsedMs / frames, cpuMsPerFrame: cpuMs / frames, navigationMsPerAction: navigationMs / 16 });
      }
    }
    return { userAgent: navigator.userAgent, adapter: { vendor: info.vendor, architecture: info.architecture, description: info.description }, heapBytes: wasm.memory.buffer.byteLength, textures, resolution: [canvas.width, canvas.height], spatialSamples: 4, temporalSamples: 1, runs };
  }, !!control);
  const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
  const measured = report.runs.filter(r => !r.warmup && r.variant === "candidate");
  const values = measured.map(r => r.completedMsPerFrame);
  const primary = median(values);
  const metrics = {
    completed_ms_per_frame: primary,
    completed_mad_ms: median(values.map(v => Math.abs(v - primary))),
    completed_best_ms: Math.min(...values), completed_worst_ms: Math.max(...values),
    cpu_ms_per_frame: median(measured.map(r => r.cpuMsPerFrame)),
    navigation_ms_per_action: median(measured.map(r => r.navigationMsPerAction)),
    requested_texture_bytes: report.textures.candidate.bytes,
    ...(control ? {
      paired_completed_ratio: median(measured.map(r => r.completedMsPerFrame / report.runs.find(c => c.run === r.run && c.variant === "control")!.completedMsPerFrame)),
      paired_cpu_ratio: median(measured.map(r => r.cpuMsPerFrame / report.runs.find(c => c.run === r.run && c.variant === "control")!.cpuMsPerFrame)),
      control_completed_ms: median(report.runs.filter(r => !r.warmup && r.variant === "control").map(r => r.completedMsPerFrame)),
    } : {}),
  };
  // Pixel captures are outside timing. Compare all candidates with the frozen baseline.
  for (const proof of capture.cases) {
    const images = await page.evaluate(async proof => {
      const reset = proof.sample.kind === "authored" || proof.sample.millis === 0;
      const plan = reset ? await (await fetch(`/${proof.scene}.json`)).text() : undefined;
      return (window as any).bench.engines.map(({engine, canvas, variant}) => {
        if (plan !== undefined) engine.load(plan);
        engine.theme(proof.theme);
        if (proof.sample.kind === "authored") engine.authored(proof.sample.seconds);
        else { if (proof.sample.action) engine.action(proof.sample.action, proof.sample.millis); engine.draw(proof.sample.millis); }
        return {variant, data: canvas.toDataURL()};
      });
    }, proof);
    for (const {variant, data} of images) await capture.save(`${variant === "control" ? "control-" : ""}${proof.id}.png`, Buffer.from(data.split(",")[1], "base64"));
  }
  if (errors.length) throw new Error(errors.join("\n"));
  await Bun.write(join(output, "report.json"), JSON.stringify({ label, metrics, ...report }, null, 2));
  await capture.complete();
  console.log(JSON.stringify({ label, metrics, adapter: report.adapter }));
  for (const [name, value] of Object.entries(metrics)) console.log(`METRIC ${name}=${value.toFixed(6)}`);
} finally {
  await browser.close();
  server.stop(true);
}
