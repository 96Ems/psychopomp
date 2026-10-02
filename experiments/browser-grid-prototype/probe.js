// THROWAWAY host, not a framework API. State and drawing stay in Rust/WASM.
import init, { GridCanvas } from "/pkg/psychopomp_browser_grid_prototype.js";

const started = performance.now();
const wasm = init();
const plans = new Map();
async function plan(name) {
  if (!plans.has(name)) {
    const response = await fetch(`/${name}.json`);
    if (!response.ok) throw new Error(`Scene ${name}: HTTP ${response.status}`);
    plans.set(name, await response.text());
  }
  return plans.get(name);
}
const glyphs = fetch("/labels.glyphs").then(r => r.arrayBuffer()).then(b => new Uint8Array(b));

class PsychopompCanvas extends HTMLElement {
  async connectedCallback() {
    this.buttons = [...this.querySelectorAll("button")];
    this.buttons.forEach(b => b.disabled = true);
    this.abort = new AbortController();
    this.frame = 0;
    this.manual = false;
    this.intersecting = true;
    this.times = [];
    this.previous = null;
    this.canvas = this.querySelector("canvas");
    this.output = this.querySelector("output");
    this.steps = this.querySelector(".step");
    this.request = () => {
      if (!this.frame && !document.hidden && this.intersecting && this.isConnected && !this.manual)
        this.frame = requestAnimationFrame(t => this.tick(t));
    };
    try {
      if (!navigator.gpu) throw new Error("This browser has no WebGPU. Showing a static native reference instead.");
      this.exports = await wasm;
      const scenes = this.querySelector('[name="scene"]');
      const requested = new URLSearchParams(location.search).get("scene");
      if ([...scenes.options].some(option => option.value === requested)) scenes.value = requested;
      this.engine = await GridCanvas.create(this.canvas, await plan(scenes.value), await glyphs);
      if (!this.isConnected) { this.engine.free(); return; }
      this.initMs = performance.now() - started;
      this.canvas.hidden = false;
      this.querySelector(".fallback").hidden = true;
      this.buttons.forEach(b => b.disabled = false);
      if (matchMedia("(prefers-reduced-motion: reduce)").matches) this.engine.action("reduce", performance.now());
      const on = (node, event, callback) => node.addEventListener(event, callback, { signal: this.abort.signal });
      for (const button of this.buttons) on(button, "click", () => this.action(button.dataset.action));
      on(this.querySelector('[name="theme"]'), "change", e => { this.engine.theme(e.target.value); this.request(); });
      on(this.querySelector('[name="scene"]'), "change", async e => {
        const revision = this.loadRevision = (this.loadRevision || 0) + 1;
        try {
          const source = await plan(e.target.value);
          if (!this.isConnected || revision !== this.loadRevision) return;
          this.engine.load(source);
          if (matchMedia("(prefers-reduced-motion: reduce)").matches) this.engine.action("reduce", performance.now());
          this.manual = false; this.previous = null; this.request();
        } catch (error) { if (revision === this.loadRevision) this.fail(error); }
      });
      on(this.canvas, "keydown", e => {
        if (e.altKey || e.metaKey || e.ctrlKey) return;
        const action = ({ ArrowRight: "next", ArrowLeft: "previous", Home: "first", End: "last", r: "replay", R: "replay-paused", p: "pause", " ": "pause", s: "slower", S: "faster", ".": "frame-next", ",": "frame-previous" })[e.key];
        if (action) { e.preventDefault(); this.action(action); }
      });
      this.visibility = () => {
        const visible = !document.hidden && this.intersecting;
        if (!visible) {
          this.wasPlaying ||= this.state?.phase === "Playing";
          this.engine.action("freeze", performance.now());
          cancelAnimationFrame(this.frame); this.frame = 0; this.previous = null;
        } else {
          if (this.wasPlaying) this.engine.action("pause", performance.now());
          this.wasPlaying = false; this.request();
        }
      };
      on(document, "visibilitychange", this.visibility);
      this.observer = new IntersectionObserver(([entry]) => {
        if (this.intersecting !== entry.isIntersecting) {
          this.intersecting = entry.isIntersecting; this.visibility();
        }
      });
      this.observer.observe(this.canvas);
      this.request();
      // Deliberate diagnostic surface for pixel/time comparison in this local probe.
      window.probe = this;
    } catch (error) { this.fail(error); }
  }
  fail(error) {
    this.querySelector('[role="alert"]').textContent = String(error);
    this.querySelector('[role="alert"]').hidden = false;
    this.canvas.hidden = true; this.querySelector(".fallback").hidden = false;
    this.buttons.forEach(b => b.disabled = true);
    cancelAnimationFrame(this.frame); this.frame = 0;
    console.error(error);
  }
  action(name) {
    try { this.manual = false; this.engine.action(name, performance.now()); this.previous = null; this.request(); }
    catch (error) { this.fail(error); }
  }
  tick(now) {
    this.frame = 0;
    try {
      const begin = performance.now();
      const state = JSON.parse(this.engine.draw(now));
      const cpu = performance.now() - begin;
      this.firstSubmitMs ??= performance.now() - started;
      if (state.phase === "Playing" && this.previous !== null) {
        this.times.push({ interval: now - this.previous, cpu });
        if (this.times.length > 600) this.times.shift();
      }
      this.previous = state.phase === "Playing" ? now : null;
      if (this.state?.step !== state.step || this.state?.title !== state.title) this.steps.textContent = `${state.step + 1}. ${state.title}`;
      this.state = state;
      this.querySelector('[data-action="slower"]').textContent = `Speed · ${state.speed}`;
      this.querySelector('[data-action="reduce"]').setAttribute("aria-pressed", String(state.reduced));
      const stats = this.stats();
      this.output.textContent = `${state.phase} · scene ${state.time.toFixed(3)}s · ${state.speed} · WASM heap ${(this.exports.memory.buffer.byteLength / 1048576).toFixed(1)} MiB\n` +
        `Startup ${this.initMs.toFixed(0)} ms (local fetch + WASM + adapter; first pipeline compilation is extra)\n` +
        `Recent ${stats.samples} moving frames: rAF interval p50 ${stats.intervalP50.toFixed(2)} / p95 ${stats.intervalP95.toFixed(2)} ms; CPU sample+submit p50 ${stats.cpuP50.toFixed(2)} ms. Not GPU time or scanout.`;
      if (state.phase === "Playing") this.request();
    } catch (error) { this.fail(error); }
  }
  stats() {
    const percentile = (key, p) => { const values = this.times.map(s => s[key]).sort((a, b) => a - b); return values[Math.floor((values.length - 1) * p)] || 0; };
    return { samples: this.times.length, intervalP50: percentile("interval", .5), intervalP95: percentile("interval", .95), cpuP50: percentile("cpu", .5), cpuP95: percentile("cpu", .95) };
  }
  sample(seconds) {
    this.manual = true; cancelAnimationFrame(this.frame); this.frame = 0;
    this.engine.authored(seconds);
    return this.canvas.toDataURL("image/png");
  }
  disconnectedCallback() {
    cancelAnimationFrame(this.frame); this.observer?.disconnect(); this.abort?.abort(); this.engine?.free();
  }
}
customElements.define("psychopomp-canvas", PsychopompCanvas);
