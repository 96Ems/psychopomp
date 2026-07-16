# Prototype Findings

## Question

Can a minimal headless Rust stack render attractive, stable technical-video frames and encode them without a browser?

## Answer

Yes. The prototype renders a three-second 1920x1080 video at 60 fps on an Apple M2 Max, reads frames back from Metal, and streams raw RGBA pixels to FFmpeg. The current 180-degree shutter render evaluates eight complete temporal samples per frame and finishes in about 25 seconds.

The output uses:

- a fullscreen WGSL shader for a flat, gradient-free dark editor and focus treatment
- `cosmic-text` for shaped, syntax-colored CommitMono line sprites
- stable line IDs and compiled before/after code snapshots
- analytic damped springs for panel movement and focus intensity
- separate layout and content progress so space opens before new lines enter
- eight complete scene samples per output frame for shutter-based motion blur
- an offscreen `Rgba8UnormSrgb` texture with GPU-to-CPU readback
- FFmpeg and `libx264` for a deterministic 1080p60 H.264 artifact

Run the prototype with:

```bash
cargo run --release
```

The default artifact is `output/kinograph-prototype.mp4`.

## What This Proves

- `wgpu` works headlessly through Metal without a window or browser.
- `cosmic-text` produces crisp CommitMono typography with per-span styling.
- Cached line sprites avoid dynamic glyph-atlas corruption and make opacity deterministic.
- Stable closing lines move to new rows without being replaced when code enters.
- GPU rendering and synchronous readback are already fast enough for a short offline-rendered prototype.
- Temporal sampling blurs moving geometry and text while settled code remains crisp.
- FFmpeg can remain a narrow encoding boundary rather than part of the rendering engine.
- Arbitrary-time analytic motion fits frame-independent rendering.

## What This Does Not Prove

- camera choreography
- a scene IR or TypeScript authoring frontend
- GPU composition of cached line sprites
- fast interactive preview

## Subsequent Findings

- Rust computation and renderer iteration do not need to share one compilation unit. A lightweight Scene Program can emit a deterministic, validated Scene Plan while a separate `kinograph-render` process retains Metal, font caches, and FFmpeg delivery. The `agent-demo` producer compiled independently, and a 200-millisecond global Render Window rendered in 0.7 seconds without weakening Rust authoring to a data-only language.
- Continuous and discrete animation are orthogonal core channels. Explicit-time set/spring events now use the same Timeline segment compiler as relative Rust Animation values, while generic `StateTrack<T>` sampling supplies previous value, age, and interval history to both authored Tasks and imported component snapshots. Task and title-card semantics remain adapters rather than core timeline variants.
- The editor-heavy hero can cross the complete process seam without flattening renderer measurements into authored pixels. Scene Plan v2 scalar values retain stable Semantic Target references; the concrete editor recipe shapes the canonical code, resolves logical range geometry, and only then compiles the shared numeric Timeline. The standalone `kinograph-hero` Scene Program and the removed direct hero path produced identical 300-frame MP4 bytes: `d9d394018b8df895d093fe075e37ddf933d6c85f24b81010b4271ac35c189801`.
- OpenCode v2 can hot-reload a newly created plugin tool into the session that created it. In one isolated conversation driven by the local `opencode-drive` v0.5.0 checkout, the first provider request did not advertise `session_greeting`; the model used the built-in `patch` tool to create `.opencode/plugins/session-tool.ts`; the location watcher loaded it; and a second prompt in the same session advertised and executed the new tool. Both tool calls and successes share session ID `ses_094a7d8c0ffePHFPA7htnul0jc` in the retained server log. The deterministic fixture, raw recording, screenshots, and log live under `experiments/opencode-v2-session-tool/` and `assets/opencode-v2-session-tool/`.
- A narration-rich Scene Plan can own authentic terminal footage and audio without a generic video graph. The corrected `opencode-v2-session-tool` plan schedules one 12.28-second edited recording, five independently placed ElevenLabs script clips, two SFX, six continuous card-pose channels, discrete recording state, and five cues. Eventful capture sections retain the 6x readability slowdown, while an unchanged raw model-wait interval from 1.667 to 2.680 seconds is omitted. The observed footage places plugin creation at 3.4 seconds, watcher confirmation at 7.0 seconds, the same-session call at 10.4 seconds, and the returned result at 12.4 seconds; narration, overlays, cues, and SFX use those event boundaries rather than equal-duration sections. The entrance is a 0.7-second, zero-bounce spring whose position, scale, Z rotation, X/Y perspective tilt, and near-edge blur tracks compile to a critically damped ratio of 1.0. The concrete terminal recipe alone consumes planned video and maps global nanoseconds into source time; FFmpeg still receives only planned audio. The verified output is 930 H.264 frames at 1920x1080/60 with a 14.907-second 44.1 kHz mono AAC mix, integrated loudness of -25.2 LUFS, true peak of -4.8 dBFS, and SHA-256 `c7d8612779d8f90c8bf6d5a5e47ca87e1eba6a6150bb587bb3c2d9f160a644e0`.
- Uniform slow motion is the wrong mapping for product footage with asynchronous waits. Applying 6x slowdown to the complete raw OpenCode capture expanded an unchanged 1.96-second model wait into 11.76 seconds of static video and left 6.44 seconds of measured silence between narration clips. Preserving slow motion around visible events while omitting the unchanged wait reduced the lesson from 22 to 15.5 seconds without removing evidence. The Scene Program now rejects narration placements that leave more than one authored second between adjacent script clips.
- Editor pixels and recorded-video pixels can share one presentation Module without a retained scene graph or renderer trait. `render/ui/card.rs` accepts packed or strided borrowed RGBA, supports nested clips, fills, strokes, fit modes, card-local overlays, and call-order composition, then owns rounded clipping, material, border, shadow, perspective, surface blur, and depth-dependent near-edge blur. A direct borrowed-source path avoids a redundant full-card raster pass for simple RGBA producers while preserving the composable closure path for cards assembled from multiple operations.
- Opaque borrowed RGBA can bypass material alpha composition without changing rendered pixels. On the same settled one-second, 1920x1080/60, eight-sample range, the direct card path improved from 26.2 seconds to 18.6 seconds; decoded frame MD5s remained identical across all 60 frames.

- Shaped glyph cluster hitboxes provide exact semantic token targets without estimating monospace advances.
- Token highlights and an independently moving pointer can target those ranges through ordinary property tracks.
- The exact Effect Institute Phosphor hand can be loaded through a reusable SVG sprite pipeline, transformed per temporal sample, and accumulated with motion blur.
- A stable line can reveal `, NotFound` by expanding the inserted spans, resolving opacity and blur, and moving the existing `>` suffix without replacing it.
- A pure Rust DSL can preserve semantic text targets until scene compilation and lower typed actor operations into the same scalar property tracks.
- Cross-media sequence, parallel, delay, and hold can schedule visual motion together with non-destructive audio or video clips.
- Transcript-bearing script clips and accompanying layer clips require distinct roles even though they share the same composition clock.
- Integer-nanosecond media time avoids source-range drift that appears immediately when decimal clip boundaries are repeatedly subtracted as floating-point values.
- The complete 31.7-second Effect Institute `effect-shows-errors` lesson can use its original Opus narration and word timing sidecar to drive structural code changes, focus ranges, pointer motion, multiple inline reveals, an error squiggle, and a celebration burst.
- FFmpeg can trim and place compiled audio clips on the composition clock while continuing to receive raw rendered video through stdin.
- A separate success sound can be scheduled as a layer clip at the same transcript cue that clears the error and reveals `VeryBadRoll`.
- Semantic targets after a collapsing inline slot must resolve against the sampled slot layout, not the backing line containing every slot alternative; otherwise highlights inherit the hidden span's width.
- Porting an Effect Institute flow requires its operation-specific motion profiles: 0.5-second bouncy cursor travel, 0.4-second critical inline parts, 0.45-second line movement, 0.3-second spotlight changes, and a 0.4-second burst entrance.
- Short semantic annotations can be first-class composition leaves: scheduling resolves their target once, arbitrary-time sampling derives normalized phase without retained particle state, and a closed recipe enum swaps prismatic bloom for focus pulse without exposing renderer plugins or drawing parameters.
- Per-clip decibel gain lets a quiet sound effect remain an immutable source asset while cue-local parallel composition synchronizes it with a semantic annotation.
- Mix level alone does not predict whether a sound effect reads under narration: the original narrow 1 kHz success tap remained masked after an 18 dB boost, while a quieter upward stereo chime with high-frequency transients stays distinct from speech.
- The complete 31.1-second `promises-only-happy-path` lesson ports without a new renderer feature: two independent inline reveals on one stable call line model `???` being replaced by `throws SomeError`, while the original narration retimes focus, pointer motion, and the optional call line through semantic cues.
- A semantic target after a hidden alternative on the same stable line needs its measured x-position adjusted by that alternative's sampled width. This is the same unresolved layout seam exposed by collapsing slots in `effect-shows-errors`, now reproduced by a second scene.
- Semantic text targets must currently use unique text within their stable line: targeting `ship` selected the earlier substring in `shipment`, while `ship(payment)` resolves the intended call. Occurrence-aware targets remain a future DSL seam.
- Applying one sampled translation, three-axis rotation, and scale to both shader geometry and the CPU-composited foreground produces a coherent 0.7-second perspective entrance whose temporal blur comes from scene motion rather than a post-process blur.
- Extreme perspective entrances expose sampling quality quickly: a five-tap cross reads as repeated glyph copies, and eight shutter samples reveal ghost contours during a 150%-to-100% pullback. A depth-weighted 3x3 Gaussian kernel plus 16 entrance samples produces a smoother near-plane blur while settled frames retain the normal eight-sample cost.
- Published narration bytes can still produce a different browser mix when a lesson flow schedules synthesized sounds separately. The Promise lesson's narration asset is byte-identical to production; its descending E5-to-C5 Tone.js cue must be represented as a cue-local layer clip.
- Effect Task states can be authored as ordinary Composition leaves while stable IDs preserve nodes across idle, running, success, failure, death, retry, and hidden intervals. Arbitrary-time sampling resets entrance age after a hidden interval rather than depending on prior rendered frames.
- Porting Pixi task pixels requires one shared fractional transform for every moving layer. Integer-snapped or independently transformed body, sweep, border, flash, and glow edges visibly separate under subpixel jitter even when their high-level spring targets match.
- Motion's width and height springs are intentionally independent in the Task recipe: running height changes over 0.2 seconds, completed result width over 0.35 seconds, and running scale returns from 0.95 without a one-frame geometry jump.
- Task pose changes and semantic state changes need independent ages even before they become separate compiled tracks. Restating an unchanged success or idle state to recenter a row must move the stable Task without replaying its flash, pulse, sound-equivalent visual accent, or content entrance.
- Product motion guidance favors one dominant action per Task transition: compression and sweep for running, result resolution for success, a brief horizontal impact for failure, and loss of energy for death. Continuous running shake and multi-axis random failure noise made state meaning less clear despite adding more motion.
- Positive `asetpts` offsets do not place delayed layer audio reliably through FFmpeg's `amix`; explicit `adelay` placement is required. A band-limited comparison against narration confirmed that the old path silently mixed task sounds at the wrong time even though the output contained an AAC stream.
- Task entrance defocus reads coherently only when blur applies to the assembled node layer. Blurring the icon independently while leaving its body and label sharp separates one stable actor into unrelated optical planes; container blur plus temporal sampling keeps defocus and motion blur distinct.
- Task success audio works better as a compact confirmation than a musical reward: a quiet two-tone interval with a short 420 ms decay leaves narration space and matches the brief result-resolution motion better than the earlier sustained triad.
- Task layout is a real motion channel rather than repeated state authoring. Dedicated pose leaves compile into velocity-preserving x/y tracks, while completed widths are measured from the same `cosmic-text` recipe used to render results and feed one centered row calculation.
- Averaging encoded sRGB bytes darkens glow and motion-blurred edges. Decoding temporal samples through a lookup table, accumulating RGB in linear light, and encoding once per output frame preserves energy without requiring a new GPU target.
- The sequential lesson reads more causally with two subdued authored links and short traveling handoff pulses. Keeping these links scene-specific avoids implying that every Task row is a generic dataflow graph.
- OpenCode Drive can provide compact deterministic product footage for Kinograph: the command-hot-reload fixture records a fixed 1200x720 viewport at 25 fps, creates `.opencode/commands/fire-the-missiles.md` while autocomplete remains open, and observes the new command 215 ms later through the real watcher and command-update path. Drive 0.5.0 preserves the loaded first frame and corrected box-drawing geometry, so the committed source needs no startup trim or glyph repair.
- Short terminal recordings do not require codec bindings or a generic media graph. Decoding once through FFmpeg into a seekable raw cache preserves arbitrary-time frame lookup, keeps the checked-in H.264 source tiny, and avoids retaining hundreds of megabytes of RGBA frames in memory.
- Product footage and motion-graphics framing have different responsibilities. Keeping the OpenCode pixels authentic while Kinograph owns rounded presentation, whole-card camera movement, the split command-file editor, and the missile payoff makes the feature demonstrable without reconstructing the TUI.
- Product footage reads more clearly when a detail move transforms the complete terminal card rather than zooming pixels inside a stationary mask. Scale, position, rotation, rounded silhouette, and shadow must remain one material; explicit bright border strokes produce repeated contour lines under temporal sampling and are better omitted.
- The command-file cause needs enough screen space to show time. A compact split-screen editor can reveal the actual checked-in `.opencode/commands/fire-the-missiles.md` fixture, transition from writing to saved, and remain beside the live autocomplete without inventing another terminal recording.
- Short generated narration can remain an ordinary script clip. The ElevenLabs line is mixed beside cue-local typing, save, launch, dual impact, and confirmation layers; no sound is renderer-triggered.
- Six concrete render targets made scene ownership a demonstrated seam rather than a hypothetical abstraction. Moving each complete choreography behind `crates/kinograph-render/src/scenes/<name>.rs::render` reduces `crates/kinograph-render/src/main.rs` to command dispatch while keeping documents, assets, targets, and sampling local; no scene trait or registry is needed.
- Repeated transcript choreography justified `Cue::at`: one cue can place Motion, Task changes, annotations, media, or nested composition at its exact start without every scene reconstructing a delay wrapper.
- Repeated structural edits justified a `CodeEdit` actor that coordinates layout/content initialization, enter/exit Motion, progress, and `CodeTransition` sampling. The hero remains an important counterexample: its layout opens before content arrives, so direct independent tracks remain part of the authoring vocabulary rather than being forced through the coordinated helper.
- The complete 30.366-second `effect-is-a-description` lesson combines three structural Code Edits and stable inline alternatives with the reusable `getTime` Task on top of editor pixels. The original narration cues drive type revelation, explicit execution, timestamp success, function equivalence, and the final `Effects are LAZY` return without a lesson-specific visual actor.
- Effect Institute maximum stability must be preserved during a port, not reconstructed from rendered snapshots. In `effect-is-a-description`, `const getTime`, ` = `, `getTime`, `)`, and the comment suffix remain stable spans while only type, implementation, call-prefix, opening-parenthesis, and comment-subject alternatives collapse or expand horizontally. Separate line IDs for each visible state made the whole definition leave and re-enter even though the screenshots looked semantically equivalent.
- Published line and variable-part motion are separate contracts. Effect Institute lines move only on `y` with a 0.45-second zero-bounce spring while opacity loss derives a 4-pixel exit blur; inline variables use a 0.4-second zero-bounce width/opacity/blur spring. Reusing a generic 0.48-second structural spring and horizontal line offset made both transitions feel unrelated to the source lesson.
- Running Tasks need one coherent charging transform rather than independent decoration. Deterministic irregular target holds and analytic damping move the body, glow, border, and sweep together, allowing temporal sampling to create motion blur without softening the static surface or label. Glow and border strength follow this unpredictable charge energy rather than independent sine waves. The energy sweep is an infinite 122-pixel train moving at 500 pixels per second, so pulses remain exactly 0.244 seconds apart instead of acquiring a larger gap when a finite three-band group wraps.
- Task content transitions need both the previous and current state in the same sampled frame. `TaskFrame` now carries the previous state's completed duration so running jitter and sweep phase settle continuously into the next state. Outgoing results fade and blur beneath incoming icons while an analytic rounded-body mask clips final destination coverage after text blur; the text keeps its natural size instead of being scaled to the springing container. Incoming icons rotate and deblur from 0.7 scale, with reset staggered 100 milliseconds so the sparkle lands on the second descending reset note. Task SFX remain cue-local composition media; `effect-is-a-description` schedules running, success, and the source-inspired reset cue beside the corresponding semantic state changes.
- Effect Institute's compiled production artifacts preserve the source DSL's useful identity seam: stable line IDs, ordered part IDs, alternate token versions, exact step times, annotations, and component snapshots. Pinning those artifacts lets Kinograph port a 30-section published corpus without re-authoring its DSL or flattening maximum-stability slots into replacement lines.
- Chapter composition is editorial scheduling, not one giant Scene. Intro and Basics retain section-local time and namespaced actors while one chapter encoder places canonical narration in manifest order around title and gap intervals. The selected non-onboarding Intro corpus is 14 sections and 450.381 seconds; Basics is 16 sections and 676.634 seconds.
- Compiled frame annotations are targets, not poses. Imported cursors use the published 0.5-second spring and enter from 40 pixels right and 30 pixels below; cursor, highlight, focus, and long-section camera tracks preserve velocity when cues retarget before settling. Highlights expand from their target center and soften through opacity-coupled edge blur. Long code sections follow active semantic targets within a clipped editor viewport rather than drawing rows outside the panel.
- Component snapshots need stable hidden actors and previous payloads. Keeping alternate sync/async and expanded/collapsed Task IDs mounted prevents row jumps, and measuring result bodies avoids clipping timestamps and object values. Reconstructing the previous snapshot's payload, rather than applying the current payload to its state name, keeps overlapping Task content transitions semantically correct.
- Published sound names remain section-local composition layers rather than renderer side effects. The 20 Intro and 7 Basics cues are scheduled at their compiled step times and mapped onto Kinograph's concrete sad, failure, success, confirmation, whoosh, alarm, and death recipes; clips are trimmed at section boundaries so they cannot alter chapter timing. Published emoji bursts retain semantic polarity through separate celebratory, danger, and sad effects rather than turning every cue into a positive bloom.
- Small pixel interfaces need composable layout before they need more chrome. Rebuilding the command-file explanation from GPUI-inspired `Bounds`, edge insets, splits, and vertical flow reduced it to one compact header, gutter, and four evenly spaced rows; the earlier explorer, breadcrumb, tab, and mode bar amplified coordinate drift without improving the explanation.

## Next Question

Can multiple recorded takes be transcribed and assembled into one non-destructive script edit whose changed word timing automatically retimes the same visual choreography?
