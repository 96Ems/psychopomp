// Capture receipts are completed only after every manifest case has been saved.
// A reused directory's old PNGs cannot substitute for missing work in this run.
import { join } from "node:path";
import { randomUUID } from "node:crypto";

export type ProofCase = {
  id: string; group: "grid" | "diagram" | "reference"; scene: string; theme: string; native: string;
  sample: { kind: "authored"; seconds: number } | { kind: "interactive"; millis: number; action: string };
};
const digest = (data: Uint8Array | string) => new Bun.CryptoHasher("sha256").update(data).digest("hex");

export async function captureRun(assets: string, output: string, group: "grid" | "diagram", variants: string[], control?: string) {
  const source = await Bun.file(join(assets, "proof-cases.json")).text();
  const manifest = JSON.parse(source) as { version: number; size: number[]; cases: ProofCase[] };
  if (manifest.version !== 1) throw new Error("Unsupported proof inventory");
  const cases = manifest.cases.filter(c => c.group === group);
  if (!cases.length || new Set(cases.map(c => c.id)).size !== cases.length) throw new Error("Invalid proof inventory");
  for (const c of cases) if (!/^[a-zA-Z0-9_.-]+$/.test(c.id) || !/^[a-zA-Z0-9_.-]+\.png$/.test(c.native)) throw new Error("Unsafe proof filename");
  const expected = cases.flatMap(c => variants.map(prefix => `${prefix}${c.id}.png`));
  const files: Record<string, string> = {};
  const native: Record<string, string> = {};
  if (group === "diagram") for (const c of cases) native[c.native] = digest(await Bun.file(join(assets, c.native)).bytes());
  const receipt = { version: 1, runId: randomUUID(), status: "running", group, variants,
    inventorySha256: digest(source), wasmSha256: digest(await Bun.file(join(assets, "pkg/psychopomp_browser_grid_prototype_bg.wasm")).bytes()),
    controlWasmSha256: control ? digest(await Bun.file(join(control, "pkg/psychopomp_browser_grid_prototype_bg.wasm")).bytes()) : undefined,
    native, files };
  await Bun.write(join(output, "proof-cases.json"), source);
  const saveReceipt = () => Bun.write(join(output, "capture-run.json"), JSON.stringify(receipt, null, 2));
  await saveReceipt();
  return {
    cases,
    async save(name: string, bytes: Uint8Array) {
      if (!expected.includes(name) || name in files) throw new Error(`Unexpected or duplicate capture: ${name}`);
      await Bun.write(join(output, name), bytes);
      files[name] = digest(bytes);
    },
    async complete() {
      if (expected.some(name => !(name in files))) throw new Error("Incomplete proof run");
      receipt.status = "complete";
      await saveReceipt();
    },
  };
}
