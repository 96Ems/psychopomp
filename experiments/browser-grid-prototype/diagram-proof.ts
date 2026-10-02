// Isolated process/profile only. Never attaches to the user's browser tabs.
import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { mkdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { captureRun } from "./proof-run";

const assets = resolve(process.env.DIAGRAM_ASSETS || join(import.meta.dir, "../../target/browser-diagram-site"));
const output = resolve(process.env.DIAGRAM_OUTPUT || join(import.meta.dir, "../../output/diagram-gpu/browser"));
await mkdir(output, { recursive: true });
const capture = await captureRun(assets, output, "diagram", [""]);
const server = Bun.serve({hostname:"127.0.0.1",port:0,async fetch(request) {
  const name=new URL(request.url).pathname.slice(1) || "diagram.html";
  if(!/^(?:pkg\/)?[a-zA-Z0-9_.-]+$/.test(name)) return new Response("Not found",{status:404});
  const file=Bun.file(join(["diagram.html","index.html","probe.js"].includes(name)?import.meta.dir:assets,name));
  if(!await file.exists()) return new Response("Not found",{status:404});
  return new Response(file,{headers:{"Content-Type":name.endsWith(".wasm")?"application/wasm":name.endsWith(".js")?"text/javascript":file.type}});
}});
const chrome = process.env.CHROME || [
  "/Users/kit/Library/Caches/ms-playwright/chromium-1234/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
  "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
].find(existsSync);
const browser=await chromium.launch({executablePath:chrome,headless:true,args:["--enable-gpu"]});
try {
  const context=await browser.newContext({viewport:{width:1920,height:1200},deviceScaleFactor:1,recordVideo:{dir:output,size:{width:1920,height:1200}}});
  const page=await context.newPage();
  const errors:string[]=[];
  page.on("pageerror",e=>errors.push(String(e)));
  page.on("console",e=>{if(e.type()==="error")errors.push(e.text());});
  await page.goto(server.url.toString());
  await page.waitForFunction(()=>!!(window as any).probe?.state);
  const report:any={browser:browser.version(),scenes:[]};
  for(const scene of new Set(capture.cases.map(c => c.scene))) {
    await page.locator('[name="scene"]').selectOption(scene);
    await page.waitForFunction(scene=>(window as any).probe.state.scene===scene,scene);
    for(const proof of capture.cases.filter(c => c.scene === scene)) {
      const png=await page.evaluate(async proof=>{
        const p=(window as any).probe;
        p.engine.theme(proof.theme);
        if(proof.sample.kind === "authored") return p.sample(proof.sample.seconds);
        const {millis,action}=proof.sample;
        if(millis===0) p.engine.load(await(await fetch(`/${proof.scene}.json`)).text());
        if(action)p.engine.action(action,millis);
        p.engine.draw(millis);return p.canvas.toDataURL();
      },proof);
      await capture.save(`${proof.id}.png`,Buffer.from(png.split(",")[1],"base64"));
    }
    // Controlled completed batches, not rAF/display FPS or isolated GPU time.
    const timing=await page.evaluate(async scene=>{
      const p=(window as any).probe;
      const plan=await(await fetch(`/${scene}.json`)).text();
      const adapter=await navigator.gpu!.requestAdapter();
      const info=adapter!.info;
      if(/swiftshader|software/i.test(`${info.vendor} ${info.architecture} ${info.description}`))throw new Error("Software GPU is not a valid result");
      const runs=[];
      for(let run=0;run<8;run++) {
        p.engine.load(plan);p.engine.draw(0);await p.engine.completed();
        let cpuMs=0,navMs=0;const started=performance.now();
        for(let frame=0;frame<120;frame++) {
          const now=frame*1000/60;
          if(frame%15===0){const start=performance.now();p.engine.action(["next","last","previous","first"][Math.floor(frame/15)%4],now);navMs+=performance.now()-start;}
          const start=performance.now();p.engine.draw(now);cpuMs+=performance.now()-start;
          if(frame%12===11)await p.engine.completed();
        }
        if(run)runs.push({completedMsPerFrame:(performance.now()-started)/120,cpuMsPerFrame:cpuMs/120,navigationMsPerAction:navMs/8});
      }
      return {adapter:{vendor:info.vendor,architecture:info.architecture,description:info.description},heap:p.exports.memory.buffer.byteLength,runs};
    },scene);
    report.scenes.push({scene,...timing});
    // Real component and rAF controls, after the explicit-clock pixel/perf work.
    await page.evaluate(()=>{const p=(window as any).probe;p.action("first");p.action("replay");p.action("next");});
    await page.waitForFunction(()=>{const p=(window as any).probe;return p.state.step===1 && p.state.time>.12;});
    await page.getByRole("button",{name:"Pause / resume",exact:true}).click();
    await page.waitForFunction(()=>(window as any).probe.state.phase==="Paused");
    const before=await page.evaluate(()=>(window as any).probe.state.time);
    await page.locator("canvas").focus();await page.keyboard.press(".");
    await page.waitForFunction(before=>Math.abs((window as any).probe.state.time-before-1/60)<1e-6,before);
    await page.locator('[name="theme"]').selectOption("black");
    await page.waitForFunction(()=>(window as any).probe.frame===0);
    if(Math.abs(await page.evaluate(()=>(window as any).probe.state.time)-before-1/60)>1e-6)throw new Error("Theme advanced clock");
    await page.locator("canvas").focus();await page.keyboard.press("s");
    await page.waitForFunction(()=>(window as any).probe.state.speed==="0.5x");
    await page.evaluate(()=>{const p=(window as any).probe;p.action("reduce");p.action("last");});
    await page.waitForFunction(()=>(window as any).probe.state.step===3 && (window as any).probe.state.phase==="Held");
    await page.locator("canvas").screenshot({path:join(output,`${scene}-black.png`)});
    await page.evaluate(()=>{const p=(window as any).probe;p.action("reduce");p.engine.action("speed:1",performance.now());});
    await page.locator('[name="theme"]').selectOption("original");
  }
  // Delayed old fetch must not replace the user's later view choice.
  await page.goto(server.url.toString());
  await page.waitForFunction(()=>(window as any).probe?.state?.scene==="daemon-merge");
  let release!:()=>void;
  const blocked=new Promise<void>(resolve=>{release=resolve;});
  await page.route("**/daemon-isometric.json",async route=>{await blocked;await route.continue();});
  const requested=page.waitForRequest("**/daemon-isometric.json");
  await page.locator('[name="scene"]').selectOption("daemon-isometric");await requested;
  await page.locator('[name="scene"]').selectOption("daemon-merge");
  const response=page.waitForResponse("**/daemon-isometric.json");release();await (await response).finished();
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  if(await page.evaluate(()=>(window as any).probe.state.scene)!=="daemon-merge")throw new Error("stale scene load won the race");
  // The grid still uses the same staged implementation after adding diagrams.
  await page.goto(new URL("index.html",server.url).toString());
  await page.waitForFunction(()=>(window as any).probe?.state?.scene==="growing-grid");
  const grid=await page.evaluate(()=>(window as any).probe.sample(17));
  await Bun.write(join(output,"grid-17.png"),Buffer.from(grid.split(",")[1],"base64"));
  await page.goto(new URL("diagram.html?scene=daemon-isometric",server.url).toString());
  await page.waitForFunction(()=>(window as any).probe?.state?.scene==="daemon-isometric");
  await page.evaluate(()=>document.querySelector("psychopomp-canvas")!.remove());
  if(errors.length)throw new Error(errors.join("\n"));
  const video=page.video();await context.close();await video?.saveAs(join(output,"navigation.webm"));
  const median=(values:number[])=>values.sort((a,b)=>a-b)[Math.floor(values.length/2)];
  report.metrics=report.scenes.map((s:any)=>{const values=s.runs.map((r:any)=>r.completedMsPerFrame);const m=median([...values]);return {scene:s.scene,completedMedianMs:m,completedMadMs:median(values.map((v:number)=>Math.abs(v-m))),cpuMedianMs:median(s.runs.map((r:any)=>r.cpuMsPerFrame)),navigationMedianMs:median(s.runs.map((r:any)=>r.navigationMsPerAction))};});
  await Bun.write(join(output,"report.json"),JSON.stringify(report,null,2));
  await capture.complete();
  console.log(JSON.stringify(report.metrics));
  console.log(`PASS: both views, ${capture.cases.length} comparison captures, pause/frame-step/speed/themes/reduced motion, scene-load race, grid regression, teardown; no browser errors`);
} finally {await browser.close();server.stop(true);}
