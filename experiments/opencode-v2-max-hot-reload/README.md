# OpenCode V2 Maximum Hot Reload Capture

This fixture drives one OpenCode v2 service, client, and session while a real Vim process edits the same project. It verifies the live surfaces used by the Psychopomp scene:

- inline commands
- agents
- project skills in a watched Git Location
- references
- providers and models
- agent permissions on the next Step
- ambient `AGENTS.md` on the next Step
- local plugin generation replacement and tool cleanup

It deliberately excludes restart-bound MCP server configuration, external TUI configuration, TUI plugin source, and skill sources outside watched project roots.

The capture was verified against OpenCode v2 commit `4a7f760d25b280fb367ac9b3a8a7ac78563e3aff` with the local OpenCode Drive v0.5.0 checkout. On macOS, `TMPDIR=/private/tmp` keeps plugin paths canonical.

## Capture

Typecheck the fixture before running it:

```bash
REPO=$(git rev-parse --show-toplevel)  # run from this repository
cd /Users/kit/code/open-source/opencode-drive
bun run src/cli/index.ts check \
  "$REPO/experiments/opencode-v2-max-hot-reload/drive.ts"
```

Run it against a checkout of the pinned OpenCode commit:

```bash
TMPDIR=/private/tmp bun run src/cli/index.ts start \
  --name psychopomp-max-hot-reload \
  --script "$REPO/experiments/opencode-v2-max-hot-reload/drive.ts" \
  --dev /private/tmp/opencode-hot-reload-audit
```

The script writes the Vim timeline to `output/opencode-v2-max-hot-reload-vim.termctrl`. Drive prints the generated OpenCode MP4 path after the script passes.

## Assemble

Render the Vim interval between the shared `ready` and `complete` cues:

```bash
termctrl video output/opencode-v2-max-hot-reload-vim.termctrl \
  --edit experiments/opencode-v2-max-hot-reload/vim-edit.json \
  --tail-ms 0 \
  --hide-cursor \
  --out output/opencode-v2-max-hot-reload-vim.mp4
```

Copy the OpenCode recording printed by Drive to `output/opencode-v2-max-hot-reload-opencode.mp4`, then align, accelerate, and combine the immutable recordings:

```bash
ffmpeg -y \
  -ss 3.58 -i output/opencode-v2-max-hot-reload-opencode.mp4 \
  -i output/opencode-v2-max-hot-reload-vim.mp4 \
  -filter_complex '[0:v]trim=duration=48.1,setpts=(PTS-STARTPTS)/2.5,scale=1110:674:force_original_aspect_ratio=decrease,pad=1114:760:(ow-iw)/2:(oh-ih)/2:color=0x080a0e[open];[1:v]trim=duration=48.1,setpts=(PTS-STARTPTS)/2.5,scale=802:708:force_original_aspect_ratio=decrease,pad=806:760:(ow-iw)/2:(oh-ih)/2:color=0x080a0e[vim];[vim][open]xstack=inputs=2:layout=0_0|806_0:fill=0x080a0e,fps=60,format=yuv420p[out]' \
  -map '[out]' -an -c:v libx264 -preset slow -crf 18 -movflags +faststart \
  assets/opencode-v2-session-tool/max-hot-reload-split.mp4
```

The committed split source is 1,153 frames at 1920x760/60, lasts 19.216667 seconds, and has SHA-256 `bf1f529dd80489c31a4f0b3bf5c3539f04e83621905b70d3cc9e11c8ebda1986`.

## Regressions Found

The audit also produced a draft red-test PR and focused issues in `anomalyco/opencode`:

- [#37427](https://github.com/anomalyco/opencode/pull/37427): draft regression-test PR
- [#37422](https://github.com/anomalyco/opencode/issues/37422): catalog update events can precede readable state
- [#37424](https://github.com/anomalyco/opencode/issues/37424): public skill import ESM initialization cycle
- [#37429](https://github.com/anomalyco/opencode/issues/37429): config-directory file changes can be ignored
- [#37421](https://github.com/anomalyco/opencode/issues/37421): MCP configuration hot reload
- [#37423](https://github.com/anomalyco/opencode/issues/37423): external TUI config and theme hot reload
- [#37425](https://github.com/anomalyco/opencode/issues/37425): TUI plugin hot reload
- [#37426](https://github.com/anomalyco/opencode/issues/37426): global, external, and HTTP skill refresh
