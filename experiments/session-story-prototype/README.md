# OpenCode + browser story — visual prototype

**Verdict:** keep the original illustrated session in the architecture article.
Kit preferred it to all three Psychopomp options. This rejected visual trial is
retained as experiment history, not a production integration or supported recipe.

```sh
bash experiments/session-story-prototype/run.sh
```

This renders three 26-second clips and serves an **isolated copy of the existing
article** at <http://127.0.0.1:5210/?prototype=sessions&variant=A>.
`--preview-only` reuses the clips; `--stills` renders six moments per option.
`ARTICLE_REPO` chooses the article checkout, `PORT` chooses the preview port, and
`PSYCHOPOMP_FONT` chooses the native CommitMono font file. FFmpeg/libx264 is required.

- **A — Paired workspaces:** cumulative session and browser remain side by side.
- **B — Focus handoff:** the session starts larger; the browser grows when used.
- **C — Tool bridge:** condensed current turn above, attached browser result below.

Left/right or the floating switcher changes variants at the same movie time.
Beat buttons and ½×/¼× support motion review. “Compare current version” displays
the existing simulation in the same article position. Playback never autostarts.
The switcher is development-only. The live article source/server is not modified.

## What is real, and what is illustrated

These are **native Rust pixels**, not CSS animation over a screenshot. The
prototype source-shares Psychopomp's exact native glyph rasterizer, UI/card
coverage/compositor, and FFmpeg subprocess encoder. Panel geometry and transcript
scroll use `PlanBuilder`, shared channel lowering, and the analytic Timeline.
The tiny private scene painter remains here; no supported recipe or public API
has been added. The emitted plans use an experiment-only recipe and cannot be
sent to the normal `psychopomp plan render/present` commands.

Delivery is 1920×1080, 60 fps, one temporal sample. It is not the supported video
export's shutter-sampled quality profile and not a performance comparison.
The generated videos can be embedded as ordinary media. **Live WASM session UI,
interruptible browser navigation, selectable canvas text, and actual session
imports have not been implemented.** Video seeking is not spring retargeting.

The condensed beats come from `src/HotReloadStory.tsx` in `opencode-architecture`.
The UI, conversation and apartment prices are illustrative. It preserves the
important distinction: OpenCode session 01 persists; switching to Code Mode
recreates the browser context. No real apartment search, message, configuration
edit, or account access occurs. No private session transcript is embedded.

## Verdict

Kit preferred the original article simulation; none of A/B/C was selected.
The initial recommendation of B was not aesthetic approval. Do not generalize
this painter into a UI Surface, port it to WebGPU, or replace the live article
on the strength of its technical checks. The commands above reproduce the trial
only when another comparison is explicitly wanted.

## Verification

All three clips decode cleanly: 1560 frames each at 1920×1080 / 60 fps.
Full-scale frames, the focus transition, and article-context screenshots were
inspected. `inspect.ts` uses disposable headless Chromium to check same-time
variant switching, speed retention, beat seeks, play/pause, no autoplay, and the
current-version round trip. Captures and reports are in
`output/session-story-prototype/review/`; media checks are beside the clips.
No active browser tab was touched. Workspace tests/formatting/strict Clippy and
the standalone prototype's Clippy/source-shared tests pass. The local glyphs and
generated assets are not published; font redistribution needs a separate check.
